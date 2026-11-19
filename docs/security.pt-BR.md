# Modelo de segurança

*Read this in [English](security.md).*

- **Criptografia ponta a ponta:** todas as conexões usam QUIC (TLS 1.3) com autenticação mútua e chaves Ed25519 via `iroh`. A comunicação tenta caminho direto P2P primeiro; quando NAT ou firewalls bloqueiam a rota direta, o `iroh` encaminha pacotes cifrados pelos relays públicos `presets::N0`, que nunca têm acesso às chaves nem ao tráfego em claro. A ponte opcional ESP32 usa MQTT sobre TLS (`8883`) com nonces Challenge-Response de uso único e payload cifrado com `HMAC-SHA256`.
- **Pareamento temporário e revogação imediata:** códigos de pareamento são de uso único, expiram em 3 minutos e são descartados após 5 tentativas inválidas. Desativar qualquer permissão ou remover um aparelho encerra as sessões ativas na hora.
- **Bloqueio do aplicativo (Android):** proteção opcional por biometria ou PIN antes de abrir ou ligar um computador salvo e ao voltar para uma sessão ativa.
- **Sandbox de arquivos:** navegação e transferências ficam restritas à pasta pessoal (`~`) com proteção contra `..` e symlinks externos, bloqueando `.ssh`, `.gnupg`, keyrings, arquivos de inicialização do shell, `systemd/user`, `autostart`, `environment.d` e diretórios internos do Oráculo. Chaves privadas e tokens são gravados com permissão `0600`.
- **Isolamento do firewall RDP:** quando o Login Remoto do sistema está ativo para destravar a tela de login, o **Oráculo Desktop** avisa se a porta `3389` estiver acessível na rede local e oferece o botão **Bloquear** (usando [`packaging/linux/harden-rdp-firewall.sh`](../packaging/linux/harden-rdp-firewall.sh)) para aceitar apenas conexões pelo túnel criptografado em `127.0.0.1`.

Para reportar vulnerabilidades de forma privada, consulte [`.github/SECURITY.md`](../.github/SECURITY.md).
