# RISC-V Test Suite Makefile
# Builds and runs RV32IMAC instruction tests

.PHONY: all test clean help

# Toolchain
AS = riscv64-unknown-elf-as
LD = riscv64-unknown-elf-ld
QEMU = qemu-riscv32

# Test names (60 total)
RV32I_TESTS := add addi sub and andi or ori xor xori \
               sll slli srl srli sra srai \
               slt slti sltu sltiu \
               beq bne blt bge bltu bgeu \
               jal jalr lui auipc \
               lb lbu lh lhu lw \
               sb sh sw \
               ld_st st_ld ma_data simple

RV32M_TESTS := mul mulh mulhsu mulhu div divu rem remu

RV32C_TESTS := rvc

RV32A_TESTS := amoadd_w amoand_w amoor_w amoxor_w amoswap_w \
               amomin_w amomax_w amominu_w amomaxu_w lrsc

ALL_TESTS := $(RV32I_TESTS) $(RV32M_TESTS) $(RV32C_TESTS) $(RV32A_TESTS)
BINARIES := $(ALL_TESTS)

# Default target
all: $(BINARIES)

# Build rules with different architectures
$(RV32I_TESTS) $(RV32M_TESTS): %: %.s
	$(AS) -march=rv32im_zifencei -mabi=ilp32 -o $@.o $<
	$(LD) -melf32lriscv --no-relax -o $@ $@.o
	rm -f $@.o

$(RV32C_TESTS): %: %.s
	$(AS) -march=rv32imc_zifencei -mabi=ilp32 -o $@.o $<
	$(LD) -melf32lriscv --no-relax -o $@ $@.o
	rm -f $@.o

$(RV32A_TESTS): %: %.s
	$(AS) -march=rv32ima_zifencei -mabi=ilp32 -o $@.o $<
	$(LD) -melf32lriscv --no-relax -o $@ $@.o
	rm -f $@.o

# Run all tests
test: $(BINARIES)
	@echo "Running all tests..."
	@echo "===================="
	@passed=0; failed=0; total=0; \
	for test in $(BINARIES); do \
		[ ! -f "$$test" ] && continue; \
		total=$$((total + 1)); \
		if $(QEMU) "$$test" > /dev/null 2>&1; then \
			echo "✓ $$test"; \
			passed=$$((passed + 1)); \
		else \
			echo "✗ $$test (exit code: $$?)"; \
			failed=$$((failed + 1)); \
		fi; \
	done; \
	echo "===================="; \
	echo "Total: $$total"; \
	echo "Passed: $$passed"; \
	echo "Failed: $$failed"; \
	[ $$failed -eq 0 ] && echo "All tests passed!" || echo "Some tests failed."; \
	exit $$failed

# Clean up binaries (keep .s files)
clean:
	rm -f $(BINARIES) $(addsuffix .o,$(BINARIES))
	@echo "Cleaned binaries (preserved .s files)"

# Show help
help:
	@echo "RISC-V Test Suite"
	@echo ""
	@echo "Targets:"
	@echo "  make all   - Build all test binaries from .s files"
	@echo "  make test  - Run all tests with qemu-riscv32"
	@echo "  make clean - Delete binaries (keep .s files)"
	@echo "  make help  - Show this message"
	@echo ""
	@echo "Test Coverage (60 total):"
	@echo "  RV32I: $(words $(RV32I_TESTS)) tests (base integer)"
	@echo "  RV32M: $(words $(RV32M_TESTS)) tests (multiply/divide)"
	@echo "  RV32C: $(words $(RV32C_TESTS)) test (compressed)"
	@echo "  RV32A: $(words $(RV32A_TESTS)) tests (atomic)"
	@echo ""
	@echo "Requirements: riscv64-unknown-elf-as, riscv64-unknown-elf-ld, qemu-riscv32"
