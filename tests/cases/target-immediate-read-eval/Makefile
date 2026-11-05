SEED := global
chosen: SEED := own
chosen: VALUE := $(eval PUBLISHED := $(SEED))$(SEED)/$(PUBLISHED)
$(info reader=[$(SEED)] published=[$(PUBLISHED)])
.PHONY: all chosen
all: chosen
chosen:;$(info value=[$(VALUE)] flavor=$(flavor VALUE) origin=$(origin VALUE))@:
