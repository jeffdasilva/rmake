$(info feature=[$(filter guile,$(.FEATURES))])
export OUT=$(if ,$(guile $(error unused)),SAFE)
all: ; @printf 'result=[%s]\n' "$$OUT"
