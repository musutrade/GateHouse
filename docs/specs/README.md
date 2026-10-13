# Technical Specifications

This directory contains detailed technical specifications for features and initiatives in the Harness-Gate project.

## What is a Technical Spec?

A technical specification (spec) is a detailed document that describes:
- **What** we're building or changing
- **Why** we're doing it (goals and motivation)
- **How** it will be implemented (technical details)
- **When** it will be delivered (timeline)
- **Success criteria** (how we measure success)

Specs are more detailed than ADRs and focus on the implementation plan rather than just the decision.

## Spec Format

Our specs follow this structure:

1. **Overview** - High-level summary
2. **Goals** - What we want to achieve
3. **Scope** - What's in/out of scope
4. **Technical Specifications** - Detailed implementation
5. **Implementation Plan** - Tasks and timeline
6. **Success Metrics** - How we measure success
7. **Risk Assessment** - Potential issues and mitigations
8. **References** - Links to ADRs and related docs

## Current Specifications

### Completed
- [Phase 1 Optimization](optimization-phase-1.md) - Quick wins for quality improvements (✅ Completed)

### Active
- [Phase 2 Optimization](optimization-phase-2.md) - Performance and code quality enhancement (🚧 In Progress)

### Planned
- Testing Strategy (Phase 3)
- Feature Expansion (Phase 3)

## Creating a New Spec

1. **Discuss the initiative** with the team first
2. **Copy a template** from an existing spec
3. **Fill in all sections** - be thorough but concise
4. **Link to relevant ADRs** - specs implement decisions
5. **Get review** before implementation
6. **Keep it updated** as implementation progresses

## Relationship to ADRs

- **ADRs** = Decisions (what we chose and why)
- **Specs** = Implementation (how we execute those decisions)

Example:
- ADR-0002: "We will optimize release builds" (decision)
- Phase 1 Spec: "Add `strip = true` to Cargo.toml" (implementation)

## Spec Status

Specs can have these statuses:
- **Draft** - Work in progress
- **In Progress** - Actively implementing
- **Completed** - Implementation done
- **Superseded** - Replaced by newer spec

Update the status at the top of the spec document as work progresses.

## Questions?

- For architectural decisions → see [ADR documentation](../adr/README.md)
- For general contribution → see [CONTRIBUTING.md](../../CONTRIBUTING.md)
- For project overview → see [README.md](../../README.md)
