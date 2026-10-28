guile=[$(0)|$(1)|$(2)]
$(info feature=[$(filter guile,$(.FEATURES))])
export OUT=$(call guile)
all: ; @printf 'result=[%s]\n' "$$OUT"
