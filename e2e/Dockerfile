FROM ubuntu:22.04

RUN apt-get update && apt-get install -y \
    openssh-server \
    && rm -rf /var/lib/apt/lists/*

# SSHサーバー設定
RUN mkdir /var/run/sshd
RUN echo 'PermitRootLogin yes' >> /etc/ssh/sshd_config
RUN echo 'AcceptEnv CSSH_PORT' >> /etc/ssh/sshd_config
RUN echo 'GatewayPorts no' >> /etc/ssh/sshd_config

# テスト用ユーザー作成
RUN useradd -m -s /bin/bash testuser
RUN echo 'testuser:testpass' | chpasswd

# SSH鍵認証用ディレクトリ
RUN mkdir -p /home/testuser/.ssh && \
    chown testuser:testuser /home/testuser/.ssh && \
    chmod 700 /home/testuser/.ssh

EXPOSE 22
CMD ["/usr/sbin/sshd", "-D"]
