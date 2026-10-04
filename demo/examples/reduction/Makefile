.SUFFIXES:
.SUFFIXES: .o .s .out .xml .input .transcript

ALLSRC=$(wildcard *.s)
START=$(filter start.s, $(ALLSRC))
AOUTSRC=$(START) $(filter-out $(START), $(ALLSRC))

STEPPER_CMD=risclet run --check-abi $(AOUTSRC)
STEPPER_DIR=inputs
STEPPER_TIMEOUT=5
STEPPER_POSTERRORLINES=15
STEPPER_GRADE=false
export STEPPER_CMD STEPPER_DIR STEPPER_TIMEOUT STEPPER_POSTERRORLINES STEPPER_GRADE

all:	step

grade:	export STEPPER_GRADE := true
grade:
	@rm -f test_detail.xml
	sh lib/stepper -- $(STEPPER_CMD)

run:
	risclet run $(AOUTSRC)

step:
	sh lib/stepper -- $(STEPPER_CMD)

trace:
	risclet trace $(AOUTSRC)

debug:
	risclet debug $(AOUTSRC)

clean:
	rm -f core *.o *.out *.xml

.PHONY: all grade run step trace debug clean
