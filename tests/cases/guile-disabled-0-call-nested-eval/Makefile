guile=[$(0)|$(1)|$(2)]
$(info feature=[$(filter guile,$(.FEATURES))])
$(eval OUT:=$(call guile,$(call guile,inner,second),tail))
$(info result=[$(OUT)])
all: ; @true
