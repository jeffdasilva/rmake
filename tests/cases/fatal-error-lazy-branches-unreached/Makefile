RESULT := $(if ,$(error first),if-ok) $(or first,$(error second)) [$(and ,$(error third))] [$(foreach x,,$(error fourth))]
all:
	@printf '%s\n' '$(RESULT)'
