-include generated.mk peer.mk
all:;@echo "$(MAKE_RESTARTS):$(VALUE)"
generated.mk peer.mk &:: input;@echo generate;printf "VALUE = ready\n" > generated.mk;cp generated.mk peer.mk
input:;@touch -d @1000000000 input
