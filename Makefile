.PHONY: w b br ebr test testmin testaurix teststm32 testcortex testrisc testall

define DEBUGBUILDNOTIF
    ____       __                   ____        _ __    __
   / __ \___  / /_  __  ______ _   / __ )__  __(_) /___/ /
  / / / / _ \/ __ \/ / / / __ `/  / __  / / / / / / __  /
 / /_/ /  __/ /_/ / /_/ / /_/ /  / /_/ / /_/ / / / /_/ /
/_____/\___/_.___/\__,_/\__, /  /_____/\__,_/_/_/\__,_/
                       /____/
endef
export DEBUGBUILDNOTIF

w:
	cargo watch -c

b:
	@echo "$$DEBUGBUILDNOTIF"
	cargo build

br:
	cargo build --release

ebr:
	cargo build --release --no-default-features

test:
	cargo test --all --no-fail-fast --features all # -- --test-threads=1

testmin:
	cargo test --no-default-features --features automotive,robotics,industrial,iot

testaurix:
	cargo test --all --features all,aurix

teststm32:
	cargo test --all --features all,stm32

testcortex:
	cargo test --all --features all,cortex-m

testrisc:
	cargo test --all --features all,riscv

testall: test testmin testaurix teststm32 testcortex testrisc