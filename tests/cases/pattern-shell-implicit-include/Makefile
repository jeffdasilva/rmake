.SECONDEXPANSION:
%.mid: SOURCE != $(eval include command.mk)/usr/bin/printf '$(INPUT)'
all: out.product
%.product: %.mid
	$(info product=[$@/$<])@:
%.mid: $$(SOURCE)
	$(info middle=[$@/$<])@:
