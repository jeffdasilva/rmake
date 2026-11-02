all:;@$(file >selected,pipe)$(info AUTH=[$(if $(filter --jobserver-auth=%,$(MAKEFLAGS)),present,absent)] TYPE=[$(if $(findstring fifo:,$(MAKEFLAGS)),fifo,pipe)]):
