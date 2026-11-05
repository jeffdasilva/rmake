NAME := GLOBAL
$(foreach NAME,PICKED,$(eval %.job: $(NAME) != /usr/bin/printf '$$(NAME)')$(info reader=[$(NAME)]))
NAME := LATE
one.job: ; $(info file=[$(PICKED)/$(GLOBAL)/$(NAME)])@:
