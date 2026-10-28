class Safepoint < Formula
  desc "Universal, local-first undo and recovery layer for coding agents"
  homepage "https://github.com/yuri-rod/agent-safepoint"
  url "https://github.com/yuri-rod/agent-safepoint/archive/refs/tags/v0.1.3.tar.gz"
  sha256 "b21f44763efc3863691827546f5273a726a87444a6b4bbe35d3b60324043c8e0"
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
