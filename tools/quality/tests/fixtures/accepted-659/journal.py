"""Durable local validation and remote delivery are separate state transitions."""
from __future__ import annotations

import json
from pathlib import Path
import sqlite3
import time
import uuid

from .common import canonical, require


class Journal:
    def __init__(self, path):
        self.db = sqlite3.connect(Path(path), isolation_level=None, timeout=10)
        self.db.row_factory = sqlite3.Row
        self.db.execute('PRAGMA journal_mode=WAL')
        self.db.execute('PRAGMA synchronous=FULL')
        self.db.executescript('''
            CREATE TABLE IF NOT EXISTS runs (
                id TEXT PRIMARY KEY, repository TEXT NOT NULL, head TEXT NOT NULL,
                epoch INTEGER NOT NULL, input TEXT NOT NULL, local TEXT NOT NULL,
                evidence TEXT, remote TEXT NOT NULL, check_id INTEGER, pr INTEGER,
                reason TEXT, created REAL NOT NULL, UNIQUE(repository,head,epoch));
            CREATE TABLE IF NOT EXISTS events (
                sequence INTEGER PRIMARY KEY, run TEXT NOT NULL,
                time REAL NOT NULL, operation TEXT NOT NULL, detail TEXT NOT NULL);
        ''')

    def close(self):
        self.db.close()

    def get(self, run):
        row = self.db.execute('SELECT * FROM runs WHERE id=?', (run,)).fetchone()
        require(row is not None, 'unknown gate run')
        value = dict(row)
        for key in ('input', 'evidence'):
            value[key] = json.loads(value[key]) if value[key] else None
        previous = self.db.execute('SELECT id FROM runs WHERE repository=? AND head=? AND epoch=?',
            (value['repository'], value['head'], value['epoch'] - 1)).fetchone()
        value['previous_id'] = previous['id'] if previous else None
        return value

    def latest(self, repository, head):
        row = self.db.execute('SELECT id FROM runs WHERE repository=? AND head=? ORDER BY epoch DESC LIMIT 1',
                              (repository, head)).fetchone()
        return self.get(row['id']) if row else None

    def create(self, inputs):
        self.db.execute('BEGIN IMMEDIATE')
        try:
            old = self.latest(inputs['repository'], inputs['head'])
            if old and old['input'] == inputs and old['remote'] != 'revoked':
                self.db.execute('COMMIT')
                return old
            active = self.db.execute("SELECT id FROM runs WHERE remote NOT IN ('merged','revoked')").fetchall()
            require(not active, 'publication slot occupied; reconcile/revoke the existing batch first')
            run = str(uuid.uuid4())
            epoch = old['epoch'] + 1 if old else 1
            self.db.execute('INSERT INTO runs VALUES (?,?,?,?,?,?,?,?,?,?,?,?)',
                            (run, inputs['repository'], inputs['head'], epoch,
                             canonical(inputs).decode(), 'pending', None, 'pending', old['check_id'] if old else None, None,
                             None, time.time()))
            self.event(run, 'created', {'epoch': epoch})
            self.db.execute('COMMIT')
            return self.get(run)
        except BaseException:
            self.db.execute('ROLLBACK')
            raise

    def event(self, run, operation, detail):
        self.db.execute('INSERT INTO events(run,time,operation,detail) VALUES (?,?,?,?)',
                        (run, time.time(), operation, canonical(detail).decode()))

    def update(self, run, **fields):
        require(fields.keys() <= {'local', 'remote', 'evidence', 'check_id', 'pr', 'reason'}, 'invalid transition field')
        values = {key: canonical(value).decode() if key == 'evidence' else value
                  for key, value in fields.items()}
        self.db.execute('BEGIN IMMEDIATE')
        try:
            self.get(run)
            self.db.execute('UPDATE runs SET ' + ','.join(key + '=?' for key in values) + ' WHERE id=?',
                            (*values.values(), run))
            self.event(run, 'update', fields)
            self.db.execute('COMMIT')
        except BaseException:
            self.db.execute('ROLLBACK')
            raise
        return self.get(run)

    def outstanding(self):
        return [self.get(row['id']) for row in self.db.execute(
            "SELECT id FROM runs WHERE remote NOT IN ('merged','revoked') ORDER BY created")]
