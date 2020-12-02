CONTAINER_RUNTIME:=$(shell command -v podman || command -v docker)
CONTAINER_RUN_FLAGS=--rm -it -v $(PWD):$(PWD):Z -w $(PWD)
SOLC_VERSION=0.7.6
#CONTAINER_IMAGE=ethereum/solc:$(SOLC_VERSION)-alpine
CONTAINER_IMAGE=evm2cpp

CONTRACT_FILE=whatever.sol

all: print-env
	# some real contracts
	$(MAKE) SOLC_VERSION=0.7.6 crowdsale.combined.json
	$(MAKE) SOLC_VERSION=0.7.6 Addition.combined.json
	$(MAKE) SOLC_VERSION=0.7.6 AdditionTest.combined.json
	$(MAKE) SOLC_VERSION=0.4.11 grid.combined.json
	$(MAKE) SOLC_VERSION=0.4.11 projectkudos.combined.json
	$(MAKE) SOLC_VERSION=0.4.19 EthConnectPonzi.combined.json
	# integer overflow contracts
	$(MAKE) SOLC_VERSION=0.4.16 BecToken.combined.json
	$(MAKE) SOLC_VERSION=0.4.18 Hexagon.combined.json
	$(MAKE) SOLC_VERSION=0.4.15 SMT.combined.json
	$(MAKE) SOLC_VERSION=0.4.15 UselessEthereumToken.combined.json
	# somehow compiles with an unparsable sourcemap
	#$(MAKE) SOLC_VERSION=0.4.21 Token.combined.json
	# some test contracts
	$(MAKE) SOLC_VERSION=0.7.6 teether_test.combined.json
	$(MAKE) crytic_complex_example.combined.json
	$(MAKE) SOLC_VERSION=0.4.16 echidna_boom_suicidal.combined.json
	$(MAKE) SOLC_VERSION=0.4.16 echidna_testme_twogates.combined.json


print-env:
	@echo "CONTAINER_RUNTIME = $(CONTAINER_RUNTIME)"
	@echo "CONTAINER_RUN_FLAGS = $(CONTAINER_RUN_FLAGS)"
	@echo "CONTAINER_IMAGE = $(CONTAINER_IMAGE)"
	@echo "SOLC_VERSION = $(SOLC_VERSION)"
	@echo "CONTRACT_FILE = $(CONTRACT_FILE)"

image.BUILT: ../docker/ubuntu.Dockerfile
	set -o pipefail; $(CONTAINER_RUNTIME) build -f $< -t $(CONTAINER_IMAGE) -q .. | tee image.BUILT || (rm image.BUILT && false)

container-enter: image.BUILT
	$(CONTAINER_RUNTIME) run $(CONTAINER_RUN_FLAGS) $(shell cat image.BUILT)

ifeq ($(CONTAINER_RUNTIME), )
%.combined.json: %.sol
	-$(RM) combined.json
	solc-$(SOLC_VERSION) --combined-json abi,bin,bin-runtime,srcmap,srcmap-runtime -o . --overwrite $< | jq > $@
else
%.combined.json: %.sol image.BUILT
	-$(RM) combined.json
	set -o pipefail; $(CONTAINER_RUNTIME) run $(CONTAINER_RUN_FLAGS) $(CONTAINER_IMAGE) \
		bash -c \
		"set -e; solc-$(SOLC_VERSION) --combined-json abi,bin,bin-runtime,srcmap,srcmap-runtime -o . --overwrite $< | jq > $@"
endif
	@(test -e combined.json && mv -f combined.json $@) || true
	@test -s $@ || (echo "empty file" $@ && rm $@ && false)

ifeq ($(CONTAINER_RUNTIME), )
build-all:
	solc-$(SOLC_VERSION) --bin --bin-runtime --abi --hashes -o . --overwrite $(CONTRACT_FILE)
else
build-all: image.BUILT
	set -o pipefail; $(CONTAINER_RUNTIME) run $(CONTAINER_RUN_FLAGS) $(CONTAINER_IMAGE) \
		solc-$(SOLC_VERSION) --bin --bin-runtime --abi --hashes -o . --overwrite $(CONTRACT_FILE)
endif

clean:
	-$(RM) *.combined.json *.bin *.bin-runtime *.sourcemap *_storage.json *.abi *.signatures

%.sol:
	$(MAKE) CONTRACT_FILE=$@ build-all 

grid.sol:
	$(MAKE) CONTRACT_FILE=$@ build-all 

crowdsale.sol:
	$(MAKE) CONTRACT_FILE=$@ build-all 

teether_test.sol:
	$(MAKE) CONTRACT_FILE=$@ build-all 

teether_test_if.sol:
	$(MAKE) CONTRACT_FILE=$@ build-all 

suicidal.sol:
	$(MAKE) CONTRACT_FILE=$@ build-all 

suicidal_cond.sol:
	$(MAKE) CONTRACT_FILE=$@ build-all 

projectkudos.sol:
	$(MAKE) CONTRACT_FILE=$@ build-all 

crytec_complex_example.sol:
	$(MAKE) CONTRACT_FILE=$@ build-all 

simpledao.sol:
	$(MAKE) CONTRACT_FILE=$@ build-all 
	
Addition.sol:
	$(MAKE) CONTRACT_FILE=$@ build-all 

AdditionTest.sol:
	$(MAKE) CONTRACT_FILE=$@ build-all 

.PHONY: clean all all-contracts-combined-json print-env build-all grid.sol crowdsale.sol teether_test.sol teether_test_if.sol suicidal_cond.sol suicidal.sol projectkudos.sol crytec_complex_example.sol simpledao.sol Addition.sol AdditionTest.sol
