TOKEN := global
chosen: SEED := own
$(foreach TOKEN,caller,$(eval chosen: VALUE := $$(TOKEN)/$$(SEED))$(info caller=[$(TOKEN)]))
$(info reader=[$(TOKEN)])
.PHONY: all chosen
all: chosen
chosen:;$(info value=[$(VALUE)] flavor=$(flavor VALUE) origin=$(origin VALUE))@:
