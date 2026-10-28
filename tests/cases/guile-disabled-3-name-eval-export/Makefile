NAME:=guile form
$(info feature=[$(filter guile,$(.FEATURES))])
export OUT=$(guile $(eval $$(NAME)=EVAL)form)
all: ; @printf 'result=[%s]\n' "$$OUT"
