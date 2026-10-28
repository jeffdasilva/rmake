NAME:=guile form
$(NAME)=FOUND
$(info feature=[$(filter guile,$(.FEATURES))])
export OUT=$(guile $(info name-effect)form)
all: ; @printf 'result=[%s]\n' "$$OUT"
