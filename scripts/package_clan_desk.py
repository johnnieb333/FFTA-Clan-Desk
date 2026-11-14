#!/usr/bin/env python3
"""Package a committed Windows Clan Desk beta; never includes game/personal data."""
import argparse,hashlib,json,pathlib,shutil,subprocess,zipfile,struct,datetime
ROOT=pathlib.Path(__file__).resolve().parents[1]
VERSION='v0.1.0-beta'
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--version',default=VERSION);a=p.parse_args()
 if any(c not in'abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789.-_'for c in a.version):raise SystemExit('Invalid version label')
 dirty=subprocess.run(['git','status','--porcelain','--untracked-files=no'],cwd=ROOT,capture_output=True,text=True,check=True).stdout
 if dirty.strip():raise SystemExit('Commit the reviewed release source before packaging. Tracked files are modified.')
 commit=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip()
 required={'FFTA-Clan-Desk.exe':'target/release/ffta-play.exe','recomp.exe':'target/release/recomp.exe','gamedb.sqlite':'gamedb.sqlite','tcc/libtcc.dll':'target/release/tcc/libtcc.dll','README.md':'README.md'}
 for folder in['Licenses']:
  for f in sorted((ROOT/folder).rglob('*')):
   if f.is_file() and '__pycache__'not in f.parts and f.suffix.lower()not in['.pyc','.dll','.exe','.so','.o']:
    required[f.relative_to(ROOT).as_posix()]=f.relative_to(ROOT).as_posix()
 for name in['README.md','COPYING','build.py','desktop_abi.c','libtcc.h','source.zip']:
  required['TinyCC/'+name]='TinyCC/'+name
 required['RELEASE-NOTES.md']='docs/CLAN-DESK-v0.1.0-beta.md'
 missing=[source for source in required.values()if not(ROOT/source).is_file()]
 if missing:raise SystemExit('Missing release input: '+', '.join(missing))
 for name in['FFTA-Clan-Desk.exe','recomp.exe','tcc/libtcc.dll']:
  b=(ROOT/required[name]).read_bytes();off=struct.unpack_from('<I',b,0x3c)[0]
  if b[:2]!=b'MZ'or b[off:off+4]!=b'PE\0\0'or struct.unpack_from('<H',b,off+4)[0]!=0x8664:raise SystemExit('Expected a Windows x86_64 binary: '+name)
 if hashlib.sha256((ROOT/'TinyCC/source.zip').read_bytes()).hexdigest()!='f660447b16d6e0efe27ba8a9122aa45ee950f84b574eeb8dd1e86bc8ad34c0ea':raise SystemExit('Matching TinyCC source checksum is required.')
 out=ROOT/'dist'/('FFTA-Clan-Desk-'+a.version+'-windows-x86_64');out.mkdir(parents=True,exist_ok=False)
 manifest=[]
 for dest,source in required.items():
  q=out/dest;q.parent.mkdir(parents=True,exist_ok=True);data=(ROOT/source).read_bytes()
  if pathlib.Path(dest).suffix.lower()in['.gba','.gb','.gbc','.sav','.srm','.state','.dump','.ppm']:raise SystemExit('Personal/game data in allowlist')
  q.write_bytes(data);manifest.append({'file':dest,'bytes':len(data),'sha256':hashlib.sha256(data).hexdigest()})
 (out/'BUILD-MANIFEST.json').write_text(json.dumps({'project':'FFTA Clan Desk','version':a.version,'commit':commit,'files':manifest},indent=2)+'\n',encoding='utf-8')
 archive=out.parent/(out.name+'.zip')
 with zipfile.ZipFile(archive,'x',zipfile.ZIP_DEFLATED,compresslevel=9)as z:
  for f in sorted(out.rglob('*')):
   if f.is_file():z.write(f,pathlib.Path(out.name)/f.relative_to(out))
 digest=hashlib.sha256(archive.read_bytes()).hexdigest();(archive.parent/(archive.name+'.sha256')).write_text(digest+'  '+archive.name+'\n',encoding='utf-8')
 print('Release commit:',commit);print('Windows ZIP:',archive);print('SHA256:',digest);print('Clean-package launch and DLL dependency checks are still required before upload.')
if __name__=='__main__':main()
