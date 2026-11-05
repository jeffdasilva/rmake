TOKEN := global
chosen: TOKEN := owner
$(foreach TOKEN,caller,$(eval chosen: VALUE := $$(TOKEN))$(info caller=[$(TOKEN)]))
.PHONY: all chosen
all: chosen
chosen:;$(info value=[$(VALUE)] flavor=$(flavor VALUE) origin=$(origin VALUE))@:
