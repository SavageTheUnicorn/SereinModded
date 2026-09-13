cask "serein" do
  version "1.0.0-nightly.8.1"
  sha256 "0df6c3bd4fae06c4249c43a114ae9e8c1372075daf3f82dc7bf0cf8bb051248a"

  url "https://github.com/ViceVerse-cz/Serein/releases/download/v#{version}/serein-v#{version}-macOS-ARM64.zip"
  name "Serein"
  desc "Experimental native Discord client"
  homepage "https://github.com/ViceVerse-cz/Serein"

  depends_on arch: :arm64
  depends_on macos: :sonoma

  app "Serein.app"
end
