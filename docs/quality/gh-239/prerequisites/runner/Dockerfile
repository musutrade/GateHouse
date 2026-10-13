FROM gh239-runner-base:41459feb2f99
RUN apt-get update && apt-get install -y --no-install-recommends git curl ca-certificates python3 openssl libicu78 gh && dpkg-query -W > /runner-packages.txt
COPY rootfs/ /
COPY runner.tar.gz /opt/runner.tar.gz
RUN echo '70920811a4f8ad4328818682bca5c6469c1c942fab52448868071d0063816613  /opt/runner.tar.gz' | sha256sum -c - && mkdir -p /opt/actions-runner && tar -xzf /opt/runner.tar.gz -C /opt/actions-runner && chmod -R a+rX /opt/actions-runner
WORKDIR /work
USER 1000:1000
ENV HOME=/work/home
LABEL purpose="GH-239 runner preparation; no registration or production acceptance"
