export LIVE
all: export LIVE := local
all:;@$(eval other: unexport LIVE := file):
other:;@$(info value=[$(LIVE)] origin=[$(origin LIVE)] flavor=[$(flavor LIVE)])printf 'other=[%s]\n' "$${LIVE-unset}"
