# https://github.com/kirich1409/boosty-cli/releases/download/v{version}/boosty-{target}.{ext}
class Boosty < Formula
  homepage "https://github.com/kirich1409/boosty-cli"
  version "0.1.0"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/kirich1409/boosty-cli/releases/download/v0.1.0/boosty-macos-aarch64.tar.gz"
    end
    on_intel do
      url "https://github.com/kirich1409/boosty-cli/releases/download/v0.1.0/boosty-macos-x86_64.tar.gz"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/kirich1409/boosty-cli/releases/download/v0.1.0/boosty-linux-aarch64.tar.gz"
    end
    on_intel do
      url "https://github.com/kirich1409/boosty-cli/releases/download/v0.1.0/boosty-linux-x86_64.tar.gz"
    end
  end

  def install
    bin.install "boosty"
  end
end
