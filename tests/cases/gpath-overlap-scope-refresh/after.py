import pathlib, sys
role, value = sys.argv[1:]
print("after=" + role + ":" + value, flush=True)
if role == "1": pathlib.Path("first-after").write_bytes(b"ready\n")
