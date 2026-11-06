export LIVE
all: unexport LIVE := local
all:;@$(eval other: LIVE += discarded):
other:;@$(info value=[$(LIVE)] origin=[$(origin LIVE)] flavor=[$(flavor LIVE)])printf 'other=[%s]\n' "$${LIVE-unset}"
