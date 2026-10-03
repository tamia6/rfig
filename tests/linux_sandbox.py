"""Real Linux sandbox assertions; use fixtures only, run as a normal user."""
import ctypes
import errno
import json
import os
from pathlib import Path
import subprocess
import tempfile

ROOT=Path(__file__).resolve().parents[1]
BIN=ROOT/'target/debug/rfig'
with tempfile.TemporaryDirectory() as temp:
    home=Path(temp); bins=home/'bin'; bins.mkdir()
    victim=home/'victim'; victim.write_text('keep'); victim.chmod(0o600)
    fixture=bins/'rfig-safety-fixture'
    fixture.write_text('''#!/usr/bin/python3
import os,socket,sys
from pathlib import Path
home=Path(os.environ['HOME'])
checks=[lambda:(home/'new').write_text('bad'), lambda:(home/'victim').write_text('bad'),
 lambda:os.chmod(home/'victim',0o777), lambda:os.unlink(home/'victim'),
 lambda:os.rename(home/'victim',home/'moved'),lambda:os.utime(home/'victim',None),
 lambda:os.setxattr(home/'victim','user.rfig',b'bad'),
 lambda:socket.socket(socket.AF_INET,socket.SOCK_STREAM),
 lambda:socket.socket(socket.AF_INET6,socket.SOCK_DGRAM),
 lambda:socket.socket(socket.AF_UNIX,socket.SOCK_STREAM),
 lambda:os.setsid(),lambda:os.setpgid(0,0)]
for check in checks:
 try:check()
 except PermissionError:continue
 except Exception as e:print('unexpected',repr(e));sys.exit(1)
 else:print('unsafe operation succeeded');sys.exit(1)
print('Commands:\\n  verified  Every isolation check passed\\nOptions:\\n  --role string  Role\\n  -v  Verbose')
'''); fixture.chmod(0o755)
    env=dict(os.environ,HOME=temp,ZDOTDIR=temp,XDG_CONFIG_HOME=str(home/'.config'),PATH=str(bins)+':/usr/bin:/bin')
    result=subprocess.run([str(BIN),'analyze',fixture.name],env=env,capture_output=True,text=True,timeout=8)
    assert result.returncode==0, result.stderr
    cache=home/'.config/rfig/fallback'/f'{fixture.name}.tsv'
    assert 'verified\tsubcommand' in cache.read_text(), result.stdout
    assert victim.read_text()=='keep' and victim.stat().st_mode&0o777==0o600
    assert not (home/'new').exists()
    # Deny Landlock detection in this child, simulating a kernel without support.
    # The real sandbox must refuse BEFORE executing the target; never run a fake sandbox.
    class Filter(ctypes.Structure):_fields_=[('code',ctypes.c_ushort),('jt',ctypes.c_ubyte),('jf',ctypes.c_ubyte),('k',ctypes.c_uint)]
    class Program(ctypes.Structure):_fields_=[('len',ctypes.c_ushort),('filter',ctypes.POINTER(Filter))]
    def no_landlock():
        libc=ctypes.CDLL(None,use_errno=True)
        code=(Filter*4)(Filter(0x20,0,0,0),Filter(0x15,0,1,444),Filter(0x06,0,0,0x50000|errno.ENOSYS),Filter(0x06,0,0,0x7fff0000))
        program=Program(4,code)
        if libc.prctl(38,1,0,0,0)!=0 or libc.prctl(22,2,ctypes.byref(program),0,0)!=0:os._exit(99)
    result=subprocess.run([str(BIN),'analyze',fixture.name],env=env,capture_output=True,text=True,preexec_fn=no_landlock,timeout=5)
    assert result.returncode!=0 and 'Landlock' in result.stderr, result.stderr
    assert victim.read_text()=='keep'
    result=subprocess.run([str(BIN),'setup','--shell','bash'],env=env,capture_output=True,text=True,preexec_fn=no_landlock,timeout=5)
    assert result.returncode==0 and 'Landlock' in result.stderr, result.stderr
    assert (home/'.config/rfig/enrich.status').read_text().startswith('unavailable\n')
print('Linux real sandbox: writes/metadata/network/session escape denied; unsupported kernel fails closed')
