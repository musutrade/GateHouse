## ADDED Requirements

### Requirement: Lightweight authenticated installation
The installer SHALL acquire only missing authenticated objects, report exact bytes, reasons and locations before network acquisition, and require an explicit unattended download policy. It SHALL preserve signatures, licenses, host ABI, compiler/LLVM, protocol, Core and measurement identity checks.

#### Scenario: Compatible environment
- **WHEN** exact required bytes exist in a compatible local environment or private shared store
- **THEN** those objects are reused and missing objects alone are acquired

#### Scenario: Incompatible measurement identity
- **WHEN** a compiler or normalization identity changes
- **THEN** incompatible baseline comparison remains blocked pending a reviewed series transition

### Requirement: Archive-free recoverable lifecycle
Installed versions SHALL retain sufficient authenticated metadata to verify the original archive without storing its full body. Upgrades SHALL prepare and verify before atomic activation. Cleanup SHALL protect current and retained rollback references and never delete external data.

#### Scenario: Tampering or interrupted migration
- **WHEN** payload, signature or receipt is corrupted, or preparation is interrupted
- **THEN** no unverified version is activated and the prior installation remains recoverable

#### Scenario: Explicit offline export
- **WHEN** the user requests an offline release export
- **THEN** the original signed archive is reconstructed and verified before being exposed

## Implementation plan
T1 design; T2–T4 component/replay implementation; T5–T6 lifecycle tests; T7 native acceptance and documentation. Each task is a bounded implementation increment in this issue, without production publication. Roll back by selecting a verified retained version; legacy format remains readable.
