# Testability Refactoring Plan

This document outlines a structured plan to refactor risclet to improve testability through dependency injection, clearer module boundaries, and better separation of concerns. The focus is on making unit tests easier to write without breaking existing functionality.

## Guiding Principles

1. **Incremental changes**: Each refactoring step should be independently testable
2. **Preserve behavior**: No functional changes during restructuring
3. **Minimize scope**: Focus on high-value improvements for testability
4. **Keep it simple**: Avoid over-engineering; this is an educational tool

## Current Architecture Issues

### Tight Coupling
- `Machine` directly owns and mutates `MemoryManager` and `CpuState`
- `Op::execute()` takes `&mut Machine`, creating a large surface area
- `Linter` requires a full `Machine` to check instructions
- ELF loading directly constructs a `Machine`
- I/O operations are hardcoded into syscall execution

### Hidden Dependencies
- System I/O (`stdin`/`stdout`) is accessed directly in `Op::execute()`
- `trace()` function couples execution with linting, I/O, and trace collection
- UI state is managed within the execution loop
- No clear separation between simulation state and execution history

### Monolithic Responsibilities
- `Machine` handles execution, tracing, memory management, and I/O
- `trace()` function does execution, linting, I/O echoing, and history management
- `Op::execute()` mixes instruction semantics with side effects

## Proposed Refactoring Steps

### Phase 1: Extract and Abstract I/O (High Priority)

**Goal**: Make I/O testable by injecting a trait-based abstraction

**Current Problem**: Syscalls directly call `io::stdin()` and `io::stdout()`, making them impossible to test without actual terminal I/O.

**Steps**:

1. **Define I/O trait** in a new `src/io_abstraction.rs`:
   ```rust
   pub trait IoProvider {
       fn read_stdin(&mut self, buffer: &mut [u8]) -> Result<usize, String>;
       fn write_stdout(&mut self, data: &[u8]) -> Result<usize, String>;
   }
   
   pub struct SystemIo;
   impl IoProvider for SystemIo { /* delegates to real I/O */ }
   
   pub struct TestIo {
       pub stdin_data: Vec<u8>,
       pub stdout_buffer: Vec<u8>,
   }
   impl IoProvider for TestIo { /* uses in-memory buffers */ }
   ```

2. **Add `io_provider` field to `Machine`**:
   - Change `Machine` to hold `Box<dyn IoProvider>`
   - Update constructor to accept I/O provider
   - Pass to syscall handlers in `Op::execute()`

3. **Update syscall implementation**:
   - Replace direct `io::stdin()` calls with `io_provider.read_stdin()`
   - Replace direct `io::stdout()` calls with `io_provider.write_stdout()`

**Value**: Enables testing of syscall behavior with predictable input/output

**Risk**: Low - purely additive change with default implementation

---

### Phase 2: Separate Instruction Execution from Side Effect Collection (High Priority)

**Goal**: Make `Op::execute()` independently testable without requiring full `Machine` state

**Current Problem**: `Op::execute()` both performs the operation AND tracks effects through mutable `Machine` state, making it hard to test individual instructions in isolation.

**Steps**:

1. **Define minimal execution context trait**:
   ```rust
   pub trait ExecutionContext {
       fn read_register(&mut self, reg: usize) -> i32;
       fn write_register(&mut self, reg: usize, value: i32);
       fn read_memory(&mut self, addr: u32, size: u32) -> Result<Vec<u8>, String>;
       fn write_memory(&mut self, addr: u32, data: &[u8]) -> Result<(), String>;
       fn read_pc(&self) -> u32;
       fn write_pc(&mut self, pc: u32) -> Result<(), String>;
       fn io_provider(&mut self) -> &mut dyn IoProvider;
       fn current_effects(&mut self) -> Option<&mut Effects>;
   }
   ```

2. **Implement `ExecutionContext` for `Machine`**:
   - Wrapper implementation that delegates to existing methods
   - No behavioral changes to `Machine` itself

3. **Create minimal test context**:
   ```rust
   pub struct TestExecutionContext {
       pub registers: [i32; 32],
       pub memory: HashMap<u32, u8>,
       pub pc: u32,
       pub io: TestIo,
   }
   impl ExecutionContext for TestExecutionContext { /* minimal impl */ }
   ```

4. **Update `Op::execute()` signature**:
   - Change from `fn execute(&self, m: &mut Machine, ...)` 
   - To `fn execute(&self, ctx: &mut dyn ExecutionContext, ...)`

**Value**: Enables unit testing of individual instructions without full machine setup

**Risk**: Medium - requires signature changes throughout, but behavior unchanged

---

### Phase 3: Extract Memory Management Interface (Medium Priority)

**Goal**: Define clear boundaries for memory operations and enable memory subsystem testing

**Current Problem**: `MemoryManager` is tightly coupled to `Machine`; segment lookup logic cannot be tested independently.

**Steps**:

1. **Define memory interface trait**:
   ```rust
   pub trait MemoryInterface {
       fn load(&self, addr: u32, size: u32) -> Result<Vec<u8>, String>;
       fn store(&mut self, addr: u32, data: &[u8]) -> Result<(), String>;
       fn load_instruction(&self, addr: u32) -> Result<(i32, u32), String>;
       fn reset(&mut self);
   }
   ```

2. **Implement trait for `MemoryManager`**:
   - Existing methods already match this interface
   - Just add trait implementation

3. **Create test memory implementation**:
   ```rust
   pub struct FlatMemory {
       data: Vec<u8>,
       base: u32,
   }
   impl MemoryInterface for FlatMemory { /* simple flat memory for testing */ }
   ```

4. **Update `Machine` to use trait**:
   - Change field from `memory: MemoryManager` to `memory: Box<dyn MemoryInterface>`
   - Update construction and usage

**Value**: Enables testing of memory edge cases (alignment, segfaults, etc.) in isolation

**Risk**: Low - existing code barely changes, mostly adds abstraction layer

---

### Phase 4: Decouple Linter from Full Machine State (Medium Priority)

**Goal**: Make linter testable with minimal state rather than full `Machine`

**Current Problem**: `Linter::check_instruction()` requires `&Machine`, but only uses a small subset of its state.

**Steps**:

1. **Define linter context trait**:
   ```rust
   pub trait LintContext {
       fn get_register(&self, reg: usize) -> i32;
       fn get_symbol_for_address(&self, addr: u32) -> Option<&String>;
       fn get_symbol_value(&self, name: &str) -> Option<u32>;
   }
   ```

2. **Implement `LintContext` for `Machine`**:
   - Simple delegation to existing methods

3. **Create minimal test lint context**:
   ```rust
   pub struct TestLintContext {
       pub registers: [i32; 32],
       pub symbols: HashMap<u32, String>,
   }
   impl LintContext for TestLintContext { /* minimal impl */ }
   ```

4. **Update `Linter::check_instruction()` signature**:
   - Change from `fn check_instruction(&mut self, m: &Machine, ...)`
   - To `fn check_instruction(&mut self, ctx: &dyn LintContext, ...)`

**Value**: Enables testing of linting rules independently from execution

**Risk**: Low - purely interface change, no logic modification

---

### Phase 5: Extract Instruction Decoding (Low-Medium Priority)

**Goal**: Make decoder testable independently from execution

**Current Problem**: `Op::new()` is coupled to the `Op` enum, making it hard to test decoding edge cases.

**Steps**:

1. **Create decoder module** (`src/decoder.rs`):
   ```rust
   pub struct InstructionDecoder;
   
   impl InstructionDecoder {
       pub fn decode(inst: i32) -> Op {
           // Move logic from Op::new() here
       }
       
       pub fn decode_compressed(inst: i32) -> Op {
           // Move from Op::decode_compressed()
       }
   }
   ```

2. **Update `Op::new()` to delegate**:
   ```rust
   pub fn new(inst: i32) -> Self {
       InstructionDecoder::decode(inst)
   }
   ```

3. **Add comprehensive decoder tests**:
   - Test each instruction format
   - Test immediate extraction
   - Test edge cases (all zeros, all ones, etc.)

**Value**: Enables systematic testing of instruction decoding

**Risk**: Low - pure refactoring, behavior unchanged

---

### Phase 6: Separate Trace Collection from Execution (Low Priority)

**Goal**: Make execution loop testable without trace collection

**Current Problem**: The `trace()` function couples execution with trace collection, linting, and I/O.

**Steps**:

1. **Extract execution loop**:
   ```rust
   pub struct Executor<'a> {
       machine: &'a mut Machine,
       instructions: &'a [Rc<Instruction>],
       addresses: &'a HashMap<u32, usize>,
   }
   
   impl<'a> Executor<'a> {
       pub fn step(&mut self) -> Result<Effects, String> {
           // Execute single instruction, return effects
       }
       
       pub fn run_until(&mut self, predicate: impl Fn(&Effects) -> bool) 
           -> Vec<Effects> {
           // Run until predicate is true
       }
   }
   ```

2. **Add middleware/observer pattern**:
   ```rust
   pub trait ExecutionObserver {
       fn before_instruction(&mut self, m: &Machine, inst: &Instruction);
       fn after_instruction(&mut self, m: &Machine, effects: &Effects);
   }
   
   pub struct LintingObserver { linter: Linter }
   pub struct TraceObserver { trace: Vec<Effects> }
   pub struct IoEchoObserver { echo: bool }
   ```

3. **Rewrite `trace()` to compose observers**:
   ```rust
   pub fn trace(...) -> Vec<Effects> {
       let mut executor = Executor::new(machine, instructions, addresses);
       let mut observers = vec![
           Box::new(TraceObserver::new()),
           Box::new(LintingObserver::new(linter)),
           Box::new(IoEchoObserver::new(echo)),
       ];
       executor.run_with_observers(&mut observers, max_steps)
   }
   ```

**Value**: Enables testing of execution without side effects, and testing observers independently

**Risk**: Medium-High - significant refactoring, but cleaner architecture

---

### Phase 7: Split Machine Construction from ELF Loading (Low Priority)

**Goal**: Enable testing of `Machine` without ELF file dependencies

**Current Problem**: `load_elf()` directly constructs `Machine`, making it hard to create test machines programmatically.

**Steps**:

1. **Create builder pattern for `Machine`**:
   ```rust
   pub struct MachineBuilder {
       segments: Vec<Segment>,
       pc_start: u32,
       global_pointer: u32,
       // ...
   }
   
   impl MachineBuilder {
       pub fn new() -> Self { /* defaults */ }
       pub fn with_segment(mut self, seg: Segment) -> Self { /* ... */ }
       pub fn with_flat_memory(mut self, size: u32) -> Self { /* ... */ }
       pub fn build(self) -> Machine { /* ... */ }
   }
   ```

2. **Add convenience constructors**:
   ```rust
   impl Machine {
       pub fn for_testing() -> Self {
           MachineBuilder::new()
               .with_flat_memory(1024 * 1024)
               .build()
       }
       
       pub fn with_program(instructions: &[i32]) -> Self {
           MachineBuilder::new()
               .with_text_segment(instructions)
               .build()
       }
   }
   ```

3. **Update `load_elf()` to use builder**:
   ```rust
   pub fn load_elf(filename: &str) -> Result<Machine, String> {
       // Parse ELF...
       MachineBuilder::new()
           .with_segments(segments)
           .with_entry_point(e_entry)
           .with_symbols(address_symbols, other_symbols)
           .build()
   }
   ```

**Value**: Dramatically simplifies test setup for integration tests

**Risk**: Low - additive change that doesn't affect existing behavior

---

### Phase 8: Improve Register File Testability (Low Priority)

**Goal**: Make register operations testable in isolation

**Current Problem**: Register state is buried in `CpuState` which is inside `Machine`.

**Steps**:

1. **Add builder and query methods to `RegisterFile`**:
   ```rust
   impl RegisterFile {
       pub fn from_array(values: [i32; 32]) -> Self { /* ... */ }
       pub fn to_array(&self) -> [i32; 32] { /* ... */ }
       pub fn set_multiple(&mut self, values: &[(usize, i32)]) { /* ... */ }
   }
   ```

2. **Add assertion helpers**:
   ```rust
   impl RegisterFile {
       pub fn assert_eq(&self, reg: usize, value: i32) {
           assert_eq!(self.get(reg), value, 
               "Register {} was {}, expected {}", 
               riscv::R[reg], self.get(reg), value);
       }
   }
   ```

**Value**: Makes register-heavy tests more readable and easier to write

**Risk**: Very Low - purely additive convenience methods

---

## Additional Improvements for Testing

### Test Utilities Module

Create `src/test_utils.rs` (behind `#[cfg(test)]`) with:

```rust
pub fn make_test_machine() -> Machine { /* ... */ }

pub fn make_test_instruction(opcode: &str, operands: &[i32]) -> Instruction { /* ... */ }

pub fn assert_register_eq(m: &Machine, reg: usize, value: i32) { /* ... */ }

pub fn assert_memory_eq(m: &Machine, addr: u32, expected: &[u8]) { /* ... */ }

pub struct InstructionBuilder {
    // Fluent API for building test instructions
}
```

### Example Test Structure

After these refactorings, tests would look like:

```rust
#[test]
fn test_add_instruction() {
    let mut ctx = TestExecutionContext::new();
    ctx.write_register(1, 10);
    ctx.write_register(2, 20);
    
    let op = Op::Add { rd: 3, rs1: 1, rs2: 2 };
    op.execute(&mut ctx, 4).unwrap();
    
    assert_eq!(ctx.read_register(3), 30);
}

#[test]
fn test_syscall_write() {
    let mut ctx = TestExecutionContext::new();
    ctx.write_register(17, 64); // write syscall
    ctx.write_register(10, 1);  // stdout
    ctx.write_register(11, 0x1000);
    ctx.write_register(12, 5);
    ctx.write_memory(0x1000, b"hello").unwrap();
    
    let op = Op::Ecall;
    op.execute(&mut ctx, 4).unwrap();
    
    assert_eq!(ctx.io.stdout_buffer, b"hello");
}

#[test]
fn test_memory_alignment_error() {
    let mut ctx = TestExecutionContext::new();
    let mut linter = Linter::new(0x2000);
    
    ctx.write_register(1, 0x1001); // unaligned address
    ctx.write_register(2, 42);
    
    let instruction = make_test_instruction(Op::Sw { rs1: 1, rs2: 2, offset: 0 });
    let mut effects = Effects::new(&instruction);
    
    let result = linter.check_instruction(&ctx, &instruction, &mut effects);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("unaligned"));
}
```

## Implementation Priority

### High Priority (Do First)
1. **Phase 1: Extract I/O** - Unblocks syscall testing
2. **Phase 2: Execution Context** - Enables instruction-level testing
3. **Phase 7: Machine Builder** - Simplifies all other tests

### Medium Priority (Do Second)
4. **Phase 3: Memory Interface** - Enables memory subsystem testing
5. **Phase 4: Linter Decoupling** - Enables linter rule testing

### Low Priority (Nice to Have)
6. **Phase 5: Decoder Extraction** - Decode testing already possible
7. **Phase 6: Trace Separation** - Most complex, least critical
8. **Phase 8: Register Utilities** - Pure convenience

## Testing Strategy

For each phase:

1. **Before refactoring**: Write characterization tests for existing behavior
2. **During refactoring**: Ensure all existing tests pass
3. **After refactoring**: Add new unit tests enabled by the refactoring
4. **Integration**: Add end-to-end tests to verify combined behavior

## Success Metrics

After completing these refactorings, we should be able to:

- [ ] Test individual instructions without creating a full `Machine`
- [ ] Test syscalls with predictable I/O (no terminal required)
- [ ] Test linter rules in isolation from execution
- [ ] Test memory operations independently
- [ ] Create test machines with minimal boilerplate
- [ ] Test execution without trace collection
- [ ] Test instruction decoding systematically

## Notes and Caveats

### What This Plan Does NOT Include

Following the guidance in FUTURE.md, this plan explicitly avoids:

- **Type system improvements** (newtype wrappers, instruction length enums)
- **Module restructuring** (src/isa/, src/machine/, etc.) - current flat structure is fine
- **Performance optimizations** (memory lookup, SIMD, etc.)
- **Feature additions** (floating point, CSRs, etc.)
- **Error type hierarchy** - String errors are fine for educational tool

### Why These Boundaries

The goal is **testability**, not perfection. The project is explicitly designed to be simple and minimal. Adding type safety or restructuring modules would:
- Increase cognitive load for contributors
- Add complexity without improving testability
- Risk breaking existing functionality
- Violate the "educational tool" mission

### Alignment with FUTURE.md

This plan is inspired by but intentionally diverges from FUTURE.md ideas:

- **Takes**: The need for better testing (section 11)
- **Adapts**: The separation of concerns ideas (section 19) but keeps them minimal
- **Ignores**: Type safety improvements (sections 1-3) as orthogonal to testability
- **Simplifies**: Module restructuring (section 10) - not needed for current codebase size

## Next Steps

Before implementing:

1. Review this plan with maintainers
2. Prioritize phases based on actual testing needs
3. Consider implementing Phase 1 + Phase 7 as MVP to unblock testing
4. Create tracking issues for each phase
5. Implement incrementally with tests at each step

## Conclusion

This refactoring plan focuses on high-value, low-risk changes that enable comprehensive unit testing while preserving the project's simplicity. By introducing minimal abstractions at key boundaries (I/O, execution context, memory, linting), we can make the codebase dramatically more testable without sacrificing its educational mission or introducing unnecessary complexity.
