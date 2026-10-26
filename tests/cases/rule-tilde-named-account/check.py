import pwd,os,sys
assert os.fsencode(sys.argv[1])==os.fsencode(pwd.getpwuid(os.getuid()).pw_dir)
print("account-home-ok")
