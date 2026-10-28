guile=[$(0)|$(1)|$(2)]
$(info feature=[$(filter guile,$(.FEATURES))])
export OUT=$(call guile,alpha,beta)
all: ; @printf 'result=[%s]\n' "$$OUT"
