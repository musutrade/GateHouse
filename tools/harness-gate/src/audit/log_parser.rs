use crate::utils::redaction::redact_text;
use anyhow::{bail, Context, Result};
use serde_json::Value;
use std::collections::VecDeque;
use std::fs::File;
use std::io::{BufRead, BufReader};

const MAX_LOG_LINE_BYTES: usize = 1024 * 1024;

fn extract_trace_id(json: &Value) -> Option<String> {
    trace_id_field(json)
        .or_else(|| json.get("fields").and_then(trace_id_field))
        .or_else(|| json.get("data").and_then(trace_id_field))
        .or_else(|| json.get("span").and_then(trace_id_field))
        .or_else(|| {
            json.get("spans")
                .and_then(Value::as_array)
                .and_then(|spans| spans.iter().rev().find_map(trace_id_field))
        })
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn trace_id_field(json: &Value) -> Option<&Value> {
    json.get("trace_id").or_else(|| json.get("request_id"))
}

fn level_of(json: &Value) -> String {
    json.get("level")
        .or_else(|| json.get("severity"))
        .and_then(Value::as_str)
        .unwrap_or("INFO")
        .to_uppercase()
}

pub(super) fn extract_error_context(input_path: &str, output_path: &str) -> Result<()> {
    let mut error_trace_id = String::new();
    let mut last_trace_id = String::new();
    for_each_bounded_line(input_path, |line| {
        let Ok(json) = serde_json::from_str::<Value>(line) else {
            return Ok(());
        };
        if let Some(tid) = extract_trace_id(&json) {
            last_trace_id = tid.clone();
            // Prefer the first ERROR trace over the last record in the file.
            if error_trace_id.is_empty() && level_of(&json) == "ERROR" {
                error_trace_id = tid;
            }
        }
        Ok(())
    })?;

    let target_trace_id = if error_trace_id.is_empty() {
        last_trace_id
    } else {
        error_trace_id
    };

    if target_trace_id.is_empty() {
        eprintln!("未找到 trace_id，降级输出原始日志尾部 30 行");
        let last_lines = get_last_n_lines(input_path, 30)?;
        crate::utils::fs::atomic_write(std::path::Path::new(output_path), last_lines, true)?;
        return Ok(());
    }

    let mut before = VecDeque::with_capacity(20);
    let mut selected = Vec::with_capacity(30);
    let mut tail = VecDeque::with_capacity(30);
    let mut error_seen = false;
    for_each_bounded_line(input_path, |line| {
        let Ok(log) = serde_json::from_str::<Value>(line) else {
            return Ok(());
        };
        if extract_trace_id(&log).as_deref() != Some(target_trace_id.as_str()) {
            return Ok(());
        }
        let compact = compact_log(&log, &target_trace_id);
        if error_seen {
            if selected.len() < 30 {
                selected.push(compact);
            }
            return Ok(());
        }
        if level_of(&log) == "ERROR" {
            error_seen = true;
            selected = before.drain(..).collect::<Vec<_>>();
            selected.push(compact);
        } else {
            if before.len() == 20 {
                before.pop_front();
            }
            before.push_back(compact.clone());
            if tail.len() == 30 {
                tail.pop_front();
            }
            tail.push_back(compact);
        }
        Ok(())
    })?;

    let mut output = if error_seen {
        selected
    } else {
        tail.into_iter().collect()
    };
    output.truncate(30);

    let json_output = serde_json::to_string_pretty(&output)?;
    crate::utils::fs::atomic_write(std::path::Path::new(output_path), json_output, true)?;
    eprintln!(
        "结构化日志已提取: {} ({} 条, trace_id={})",
        redact_text(output_path),
        output.len(),
        redact_text(&target_trace_id)
    );
    Ok(())
}

fn compact_log(log: &Value, trace_id: &str) -> Value {
    let timestamp = log
        .get("timestamp")
        .or_else(|| log.get("time"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let level = level_of(log);
    let target = log
        .get("target")
        .or_else(|| log.get("module"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let fields = log.get("fields").or_else(|| log.get("data"));
    let msg = fields
        .and_then(|f| f.get("message").or_else(|| f.get("msg")))
        .and_then(Value::as_str)
        .unwrap_or("");
    let error = fields
        .and_then(|f| f.get("error"))
        .or_else(|| log.get("error"))
        .map(|v| {
            v.as_str()
                .map(str::to_string)
                .unwrap_or_else(|| v.to_string())
        })
        .unwrap_or_default();
    serde_json::json!({
        "timestamp": redact_text(timestamp),
        "level": redact_text(&level),
        "target": redact_text(target),
        "msg": redact_text(msg),
        "error": redact_text(&error),
        "trace_id": redact_text(trace_id),
    })
}

fn get_last_n_lines(path: &str, n: usize) -> Result<String> {
    let mut lines = VecDeque::with_capacity(n);
    for_each_bounded_line(path, |line| {
        if lines.len() == n {
            lines.pop_front();
        }
        lines.push_back(redact_text(line));
        Ok(())
    })?;
    Ok(lines.into_iter().collect::<Vec<_>>().join("\n"))
}

fn for_each_bounded_line<F>(path: &str, mut callback: F) -> Result<()>
where
    F: FnMut(&str) -> Result<()>,
{
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let mut line = Vec::with_capacity(MAX_LOG_LINE_BYTES.min(64 * 1024));
    loop {
        line.clear();
        if !read_bounded_line(&mut reader, &mut line)? {
            break;
        }
        let text = std::str::from_utf8(&line).context("log line is not UTF-8")?;
        callback(text.trim_end_matches(['\r', '\n']))?;
    }
    Ok(())
}

fn read_bounded_line(reader: &mut BufReader<File>, line: &mut Vec<u8>) -> Result<bool> {
    loop {
        let chunk = reader.fill_buf()?;
        if chunk.is_empty() {
            return Ok(!line.is_empty());
        }
        let newline = chunk.iter().position(|byte| *byte == b'\n');
        let take = newline.map_or(chunk.len(), |index| index + 1);
        if line.len().saturating_add(take) > MAX_LOG_LINE_BYTES {
            bail!(
                "log line exceeds {} bytes; refusing unbounded parse",
                MAX_LOG_LINE_BYTES
            );
        }
        line.extend_from_slice(&chunk[..take]);
        reader.consume(take);
        if newline.is_some() {
            return Ok(true);
        }
    }
}
