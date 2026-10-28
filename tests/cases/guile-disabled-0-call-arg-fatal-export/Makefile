$(info feature=[$(filter guile,$(.FEATURES))])
export OUT=$(call guile,$(error arg-fail))
all: ; @printf 'result=[%s]\n' "$$OUT"
