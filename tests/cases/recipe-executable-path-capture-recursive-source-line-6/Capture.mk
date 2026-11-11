CMD = $(shell __rmake_review_missing_command__)
$(info first=[$(CMD)] status=[$(.SHELLSTATUS)])
$(info second=[$(CMD)] status=[$(.SHELLSTATUS)])
all: ; @:
