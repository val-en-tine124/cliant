"""This python script will create a feature based i.e vertical slice architecture pattern for any project in it directory.\n
This script should be in a features folder.
"""
import sys
from pathlib import Path
features_files=["mod.rs","cli.rs","handler.rs",]

def create_features(feature_name:str):
    cwd=Path.cwd()
    if cwd.__str__().split("/")[-1] != "features":
        _path=cwd.parents[0].__str__().split("/")[-1]
        print(f"Warning: present working dir {cwd} immediate parent {_path} is not features dir !, current working directory should be features.")
    base_path=Path(cwd.joinpath(feature_name))
    base_path.mkdir(exist_ok=True)
    for file in features_files:
        print(f"creating {file}...")
        base_path.joinpath(file).touch(exist_ok=True)

create_features("custom_feature")
    
def main():
    features=sys.argv[1:]
    if not features:
        print("Pass a feature name to this script.")
    for arg in features:
        try:
            create_features(arg)
        except Exception as e:
            print(f"An exception occurred :{e}")

if __name__ == "__main__":
    main()