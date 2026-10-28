guile=[$(0)|$(1)|$(2)]
$(info feature=[$(filter guile,$(.FEATURES))])
.SECONDEXPANSION:
all: $$(info result=[$$(call guile,$$(call guile,inner,second),tail)])
	@true
