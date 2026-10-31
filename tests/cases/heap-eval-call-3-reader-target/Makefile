walk=$(if $(1),$(eval SEEN+=x)x$(call walk,$(wordlist 2,9999,$(1))))
SEEN:=
$(info VALUE=[$(call walk,word word word)])$(info COUNT=[$(words $(SEEN))])
.PHONY: all
all:;
