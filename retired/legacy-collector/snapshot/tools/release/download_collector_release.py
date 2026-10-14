"""Independent release downloads with HTTP/1.1, bounded ranges and retry caches."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess

import collector_assets as assets
import friendly_collector_install as download
import install_collector as lifecycle


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--tag',required=True)
    parser.add_argument('--expected',type=Path,required=True)
    parser.add_argument('--output',type=Path,required=True)
    args=parser.parse_args()
    args.output.mkdir(parents=True,exist_ok=True)
    cache=args.output.with_name(args.output.name+'-cache').absolute()
    releases=json.loads(subprocess.check_output(['gh','api','repos/'+assets.REPOSITORY+'/releases?per_page=100']))
    release=next(r for r in releases if r['tag_name']==args.tag)
    with lifecycle.locked(cache):
        for name in assets.ASSETS+assets.CONTROL:
            local=args.expected/name;assets.regular(local)
            row=next(a for a in release['assets'] if a['name']==name)
            assets.require(row['size']==local.stat().st_size and row['digest']=='sha256:'+assets.sha(local),
                           'server asset differs from approved output')
            cfg='header = "Authorization: Bearer '+os.environ['GH_TOKEN']+'"\nheader = "Accept: application/octet-stream"\n'
            response=subprocess.run(['curl','--config','-','--http1.1','--silent','--show-error',
                '--connect-timeout','15','--max-time','60','--dump-header','-','--output','/dev/null',row['url']],
                input=cfg,text=True,capture_output=True,check=True)
            url=next(line.split(': ',1)[1] for line in response.stdout.splitlines() if line.lower().startswith('location: '))
            saved=download.fetch({'name':name,'size':row['size'],'sha256':assets.sha(local),'url':url},'',cache)
            target=args.output/name
            if target.exists() or target.is_symlink(): assets.regular(target)
            shutil.copyfile(saved,target)


if __name__=='__main__': main()
