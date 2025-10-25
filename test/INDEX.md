# Test Suite Documentation Index

All documentation for the RISC-V test suite is located in this directory.

## Quick Navigation

### For New Users
1. **[GETTING_STARTED.md](GETTING_STARTED.md)** - Start here! Prerequisites and first steps
2. **[README.md](README.md)** - Test suite overview and usage
3. **[QUICK_REFERENCE.md](QUICK_REFERENCE.md)** - Command cheat sheet

### For Developers
4. **[TEST_GENERATION.md](TEST_GENERATION.md)** - How tests are generated from upstream
5. **[UPSTREAM_SOURCES.md](UPSTREAM_SOURCES.md)** - Restoring and extending test sources
6. **[TEST_SUITE.md](TEST_SUITE.md)** - Complete technical documentation

### Reference
7. **[FINAL_SUMMARY.md](FINAL_SUMMARY.md)** - Project accomplishments and technical details

## Documentation Overview

| File | Purpose | Audience |
|------|---------|----------|
| GETTING_STARTED.md | Prerequisites, first steps, troubleshooting | New users |
| README.md | Overview, usage, test coverage | All users |
| QUICK_REFERENCE.md | Command reference, examples | All users |
| TEST_GENERATION.md | Test preprocessing and generation process | Developers |
| UPSTREAM_SOURCES.md | Restoring sources, adding extensions | Developers |
| TEST_SUITE.md | Complete technical documentation | Developers |
| FINAL_SUMMARY.md | Project summary and accomplishments | Reference |

## Quick Links

**Want to:**
- Run tests? → [README.md](README.md#usage)
- Install tools? → [GETTING_STARTED.md](GETTING_STARTED.md#prerequisites)
- Add new tests? → [UPSTREAM_SOURCES.md](UPSTREAM_SOURCES.md#adding-new-extensions)
- Understand test structure? → [TEST_SUITE.md](TEST_SUITE.md#test-structure)
- Debug a failure? → [GETTING_STARTED.md](GETTING_STARTED.md#example-debugging-a-failed-test)
- See what's tested? → [README.md](README.md#test-coverage)

## Test Suite Status

✅ **50/50 tests passing (100%)**

- 40 RV32I base instruction tests
- 8 RV32M multiply/divide tests
- 1 RV32C compressed instruction test
- 1 test skipped (fence_i - not applicable)

---

*Start with [GETTING_STARTED.md](GETTING_STARTED.md) if this is your first time using the test suite.*
