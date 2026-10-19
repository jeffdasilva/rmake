MAKEOVERRIDES=$(warning warn)Y=new
$(info parsed-X=<$(value X)>|$(origin X)|$(flavor X))
$(info parsed-Y=<$(value Y)>|$(origin Y))
$(info overrides=<$(value MAKEOVERRIDES)>|$(origin MAKEOVERRIDES)|$(flavor MAKEOVERRIDES))
$(info hidden=<$(-*-command-variables-*-)>|$(origin -*-command-variables-*-)|$(flavor -*-command-variables-*-))
all:;@$(info recipe-X=<$(value X)>|$(X)|$(origin X)|$(flavor X))$(info recipe-Y=<$(value Y)>|$(origin Y)):
