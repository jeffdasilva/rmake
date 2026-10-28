NAME:=guile form
$(NAME)=SHELL
$(info feature=[$(filter guile,$(.FEATURES))])
.SECONDEXPANSION:
all: $$(info result=[$$(guile $$(shell printf %s form))])
	@true
