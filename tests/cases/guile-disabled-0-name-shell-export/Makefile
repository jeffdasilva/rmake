NAME:=guile form
$(NAME)=SHELL
$(info feature=[$(filter guile,$(.FEATURES))])
export OUT=$(guile $(shell printf %s form))
all: ; @printf 'result=[%s]\n' "$$OUT"
