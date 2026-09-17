cask "serein" do
  version "1.0.0-nightly.20260917.36"
  sha256 "eef70854085176c775c47eac9a2cdfec5db3c28d5506608f82d4104965970421"

  url "https://github.com/ViceVerse-cz/Serein/releases/download/v#{version}/serein-v#{version}-macOS-ARM64.zip"
  name "Serein"
  desc "Experimental native Discord client"
  homepage "https://github.com/ViceVerse-cz/Serein"

  depends_on arch: :arm64
  depends_on macos: :sonoma

  app "Serein.app"
end
