class Safepoint < Formula
  desc "Universal, local-first undo and recovery layer for coding agents"
  homepage "https://github.com/yuri-rod/agent-safepoint"
  url "https://github.com/yuri-rod/agent-safepoint/archive/refs/tags/v0.1.2.tar.gz"
  sha256 "3544e263517a5d1c378a53e74300800e2464dbecddea6108e2c46a45c117544a"
  license "MIT"
  head "https://github.com/yuri-rod/agent-safepoint.git", branch: "main"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args
  end

  test do
    assert_match "safepoint", shell_output("#{bin}/safepoint --version")
  end
end
