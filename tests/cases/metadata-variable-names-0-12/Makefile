$(info names=[$(.VARIABLES)] value=[$(value .VARIABLES)] origin=$(origin .VARIABLES) flavor=$(flavor .VARIABLES))
undefine .VARIABLES
.VARIABLES=custom
A=new
$(info names=[$(.VARIABLES)] value=[$(value .VARIABLES)] origin=$(origin .VARIABLES) flavor=$(flavor .VARIABLES))
all:
	@$(info recipe=[$(.VARIABLES)] value=[$(value .VARIABLES)])true
