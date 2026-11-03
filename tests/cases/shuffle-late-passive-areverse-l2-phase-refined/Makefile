$(file >seen,anchor)
GNUMAKEFLAGS = --shuffle=2
$(info INITIAL=[$(filter --shuffle=%,$(MAKEFLAGS))])
.PHONY: all a b c
all:a b c
	@$(file >>seen,all)$(info FINAL=[$(filter --shuffle=%,$(MAKEFLAGS))] PLUS=[$+]):
a:;@$(file >>seen,a)$(info a):
b:;@$(file >>seen,b)$(info b):
c:;@$(file >>seen,c)$(info c):
