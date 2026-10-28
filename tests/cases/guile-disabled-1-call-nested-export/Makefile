guile=[$(0)|$(1)|$(2)]
$(info feature=[$(filter guile,$(.FEATURES))])
export OUT=$(call guile,$(call guile,inner,second),tail)
all: ; @printf 'result=[%s]\n' "$$OUT"
