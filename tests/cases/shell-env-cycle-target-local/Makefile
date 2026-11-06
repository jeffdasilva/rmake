unexport K
all: K = $(shell printf '%s' "$$CHAIN")
all: export CHAIN = value/$(K)
all:;@printf 'read=[%s]\n' '$(K)'
