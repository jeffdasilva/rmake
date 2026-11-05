TOKEN := global
.PHONY: all caller chosen
all: caller chosen
caller: TOKEN := caller
caller:;$(foreach TOKEN,local,$(eval chosen: VALUE := $$(TOKEN))$(info loop=[$(TOKEN)]))$(info caller=[$(TOKEN)])@:
chosen:;$(info value=[$(VALUE)] flavor=$(flavor VALUE) origin=$(origin VALUE))@:
