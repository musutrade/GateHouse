"""Bootstrap entry: require production trust even for selection and rollback."""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import collector_assets as assets
import install_collector


def main():
    # Reuse the lifecycle parser; inspect its mandatory trust argument first.
    import argparse
    parser = argparse.ArgumentParser(add_help=False)
    parser.add_argument('--trust', type=Path, required=True)
    args, _ = parser.parse_known_args()
    assets.require(assets.read(args.trust).get('schema') == 'rust-collector-host-trust/v2',
                   'production bootstrap requires independently provisioned v2 trust')
    install_collector.main()


if __name__ == '__main__':
    main()
