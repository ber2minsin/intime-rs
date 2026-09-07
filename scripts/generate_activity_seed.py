#!/usr/bin/env python3
"""Generate comprehensive category + activity_rule seed SQL for migration 0005."""

from __future__ import annotations

from pathlib import Path

OUT = Path(__file__).resolve().parents[1] / "crates/intime-storage/migrations/0005_sessions_and_rules.up.sql"

# (slug, name, description, occupation_tags_json)
CATEGORIES: list[tuple[str, str, str, str]] = [
    ("code_editing", "Code editing", "IDEs and text editors for software", '["developer"]'),
    ("ai_coding", "AI-assisted coding", "AI coding tools and agent IDEs", '["developer"]'),
    ("terminal", "Terminal", "Shells and terminal emulators", '["developer","sysadmin"]'),
    ("database", "Database", "DB clients, warehouses, query UIs", '["developer","data"]'),
    ("devops", "DevOps / cloud", "Infra, CI, containers, cloud consoles", '["developer","sysadmin"]'),
    ("api_testing", "API testing", "REST/GraphQL clients and mocks", '["developer","qa"]'),
    ("browsing", "Other web browsing", "Generic browser fallback when no site-specific category matches", '["general"]'),
    ("web_search", "Web search", "Search engines and query result pages", '["general"]'),
    ("media_streaming_official", "Watching series / streaming", "Licensed TV, movies, and video streaming (Netflix, YouTube watch, Prime, …)", '["general","media"]'),
    ("media_streaming_unofficial", "Unofficial streaming", "Free/unofficial film and TV sites", '["general","media"]'),
    ("media_local", "Local media players", "Local video/audio players and servers", '["general","media"]'),
    ("music_listening", "Music listening", "Music apps and radio", '["general","media"]'),
    ("live_streaming", "Live streaming", "Twitch, Kick, live broadcasts", '["general","creator"]'),
    ("social_short", "Short-form video", "TikTok, Reels, Shorts, Stories", '["marketing","creator","general"]'),
    ("social_long", "Social media", "X/Twitter, Instagram, Reddit, LinkedIn, YouTube home/channels, forums", '["marketing","creator","general"]'),
    ("social_messaging", "Social messaging", "DMs and consumer messengers", '["general"]'),
    ("content_creation", "Content creation", "Creator studios, editors, schedulers", '["creator","marketing"]'),
    ("meetings", "Meetings", "Video calls and webinars", '["office","general"]'),
    ("communication", "Work chat", "Team chat (Slack, Teams chat, Discord work)", '["office"]'),
    ("email", "Email", "Mail clients and webmail", '["office","general"]'),
    ("office_docs", "Documents", "Word processors and Google Docs", '["office"]'),
    ("spreadsheets", "Spreadsheets", "Excel, Sheets, Numbers", '["office","finance","data"]'),
    ("presentations", "Presentations", "Slides and pitch decks", '["office","sales"]'),
    ("notes_wiki", "Notes & wiki", "Notion, Obsidian, Confluence, wikis", '["office","knowledge"]'),
    ("bi_analytics", "BI & analytics", "Power BI, Tableau, Looker, Metabase", '["data","management","finance"]'),
    ("project_mgmt", "Project management", "Jira, Linear, Asana, Monday", '["office","developer","management"]'),
    ("crm_sales", "CRM & sales", "Salesforce, HubSpot, Pipedrive", '["sales","management"]'),
    ("hr_people", "HR & people", "HRIS, recruiting, payroll UIs", '["hr","management"]'),
    ("finance_accounting", "Finance & accounting", "ERP, bookkeeping, expense tools", '["finance","management"]'),
    ("legal_compliance", "Legal & compliance", "Contract, e-sign, GRC tools", '["legal","management"]'),
    ("exec_management", "Executive & strategy", "OKR, board, strategy, exec dashboards", '["management"]'),
    ("customer_support", "Customer support", "Helpdesks and ticketing", '["support","office"]'),
    ("design_2d", "2D design", "UI/UX, illustration, raster/vector", '["design"]'),
    ("vfx_3d", "3D / VFX / DCC", "3D, VFX, game engines, CGI", '["vfx","gamedev","design"]'),
    ("video_editing", "Video editing", "NLE and motion graphics", '["creator","media","design"]'),
    ("audio_production", "Audio production", "DAWs and audio tools", '["creator","media"]'),
    ("education", "Education & learning", "LMS, courses, tutoring", '["education","general"]'),
    ("research", "Research", "Papers, libraries, reference managers", '["research","education"]'),
    ("healthcare", "Healthcare", "EHR, telehealth, clinical tools", '["healthcare"]'),
    ("reading", "Reading", "PDFs, ebooks, docs, knowledge bases", '["general","research"]'),
    ("file_management", "Files & cloud drive", "File managers and sync UIs", '["general","office"]'),
    ("calendar_tasks", "Calendar & tasks", "Calendars and personal task apps", '["office","general"]'),
    ("banking_finance_personal", "Personal banking", "Banks, wallets, investing apps", '["general","finance"]'),
    ("shopping", "Shopping", "E-commerce and marketplaces", '["general"]'),
    ("travel_maps", "Travel & maps", "Maps, booking, transit", '["general"]'),
    ("gaming", "Gaming", "Games and launchers", '["general"]'),
    ("system_settings", "System & settings", "OS settings, package managers, stores", '["general","sysadmin"]'),
    ("ai_chat_general", "General AI chat", "ChatGPT, Claude, Gemini (non-coding)", '["general","office"]'),
    ("unknown", "Uncategorized", "No rule matched; refine with user/LLM rules", '["general"]'),
]

# Deduped rules: key = (field, op, pattern_lower) -> (slug, priority)
rules: dict[tuple[str, str, str], tuple[str, int]] = {}


def add(slug: str, field: str, op: str, pattern: str, priority: int) -> None:
    pattern = pattern.strip()
    if not pattern:
        return
    key = (field, op, pattern.lower())
    prev = rules.get(key)
    if prev is None or priority > prev[1]:
        rules[key] = (slug, priority)


def products(slug: str, names: list[str], p: int) -> None:
    for n in names:
        add(slug, "product_name", "contains", n, p)
        add(slug, "display_name", "contains", n, p - 2)


def aumids(slug: str, names: list[str], p: int) -> None:
    for n in names:
        add(slug, "aumid", "contains", n, p)


def companies(slug: str, names: list[str], p: int) -> None:
    for n in names:
        add(slug, "company", "contains", n, p)


def titles(slug: str, needles: list[str], p: int) -> None:
    for n in needles:
        add(slug, "window_title", "contains", n, p)


def urls(slug: str, domains: list[str], p: int) -> None:
    for d in domains:
        add(slug, "url", "contains", d, p)
        # Browser title bars usually include the site name even when URL enrichment misses.
        add(slug, "window_title", "contains", d, p - 5)


def paths(slug: str, needles: list[str], p: int) -> None:
    for n in needles:
        add(slug, "executable_path", "contains", n, p)


def build() -> None:
    # --- Developer ---
    products(
        "code_editing",
        [
            "visual studio code", "vscode", "visual studio", "sublime text", "notepad++",
            "neovim", "nvim", "helix", "kakoune", "emacs", "spacemacs", "doom emacs",
            "goland", "pycharm", "webstorm", "phpstorm", "rubymine", "intellij", "clion",
            "rider", "dataspell", "android studio", "xcode", "zed", "nova", "bbedit",
            "textmate", "kate", "geany", "brackets", "atom", "lapce", "fleet", "theia",
            "netbeans", "eclipse", "qt creator", "code::blocks", "arduino ide",
            "rider", "appcode", "studio 3t",
        ],
        95,
    )
    aumids("code_editing", ["Code", "code-oss", "codium", "sublime_text", "nvim", "emacs"], 90)
    add("code_editing", "product_name", "equals", "vim", 95)
    add("code_editing", "product_name", "equals", "gvim", 95)
    add("code_editing", "product_name", "equals", "code", 80)
    titles("code_editing", ["visual studio code", " - code - ", "intellij idea", "android studio"], 75)
    paths("code_editing", ["/usr/bin/code", "Visual Studio Code", "JetBrains"], 70)

    products(
        "ai_coding",
        ["cursor", "windsurf", "copilot", "github copilot", "aider", "continue.dev", "tabnine",
         "codeium", "sourcegraph cody", "amazon q developer", "jetbrains ai", "replit agent",
         "devin", "sweep", "cursor.sh"],
        120,
    )
    aumids("ai_coding", ["Cursor", "Windsurf"], 118)
    titles("ai_coding", [" — cursor", " - cursor", "windsurf", "github copilot"], 110)
    urls("ai_coding", ["cursor.com", "windsurf.com", "github.com/copilot", "replit.com"], 105)

    products(
        "terminal",
        ["alacritty", "kitty", "wezterm", "foot", "ghostty", "gnome-terminal", "konsole",
         "xfce4-terminal", "terminator", "tilix", "guake", "yakuake", "hyper", "warp",
         "windows terminal", "iterm", "iterm2", "tabby", "electerm", "fluent terminal",
         "powershell", "pwsh", "cmd.exe", "windows powershell", "fluent terminal"],
        100,
    )
    aumids("terminal", ["Alacritty", "kitty", "org.wezfurlong.wezterm", "com.mitchellh.ghostty"], 100)
    titles("terminal", ["bash", "zsh", "fish - ", "powershell"], 40)

    products(
        "database",
        ["dbeaver", "datagrip", "pgadmin", "tableplus", "beekeeper", "sequel ace", "sequel pro",
         "mysql workbench", "azure data studio", "ssms", "sql server management", "toad",
         "navicat", "heidisql", "dbvisualizer", "squirrel sql", "redis insight", "redisinsight",
         "mongodb compass", "robo 3t", "nosqlbooster", "studio 3t", "snowflake", "bigquery",
         "trino", "presto", "duckdb", "datasette"],
        110,
    )
    titles("database", ["dbeaver", "datagrip", "pgadmin", "mysql workbench", "mongodb compass"], 100)
    urls(
        "database",
        ["console.cloud.google.com/bigquery", "app.snowflake.com", "cloud.mongodb.com",
         "redis.com", "planetscale.com", "neon.tech", "supabase.com/dashboard", "turso.tech",
         "railway.app", "aiven.io", "cockroachlabs.cloud"],
        115,
    )

    products(
        "devops",
        ["docker desktop", "docker", "podman", "kubernetes", "lens", "k9s", "rancher",
         "terraform", "pulumi", "vagrant", "virtualbox", "vmware", "multipass", "minikube"],
        95,
    )
    urls(
        "devops",
        [
            "console.aws.amazon.com", "aws.amazon.com", "portal.azure.com", "azure.microsoft.com",
            "console.cloud.google.com", "cloud.google.com", "cloud.digitalocean.com",
            "cloud.linode.com", "my.ionos.com", "dash.cloudflare.com", "vercel.com",
            "netlify.com", "fly.io", "heroku.com", "render.com", "railway.app",
            "app.terraform.io", "app.pulumi.com", "grafana.com", "prometheus.io",
            "app.datadoghq.com", "newrelic.com", "sentry.io", "pagerduty.com",
            "statuspage.io", "github.com/actions", "gitlab.com", "circleci.com",
            "app.travis-ci.com", "buildkite.com", "jenkins", "argocd", "harbor",
            "quay.io", "hub.docker.com", "ghcr.io", "eks.amazonaws.com",
            "console.hetzner.cloud", "ovh.com/manager", "scaleway.com", "vultr.com",
            "app.octopus.com", "spinnaker", "fluxcd", "kustomize",
        ],
        110,
    )
    titles("devops", ["aws console", "azure portal", "gcp console", "kubernetes", "grafana", "datadog"], 100)

    products(
        "api_testing",
        ["postman", "insomnia", "hoppscotch", "bruno", "paw", "httpie", "rest client", "soapui", "katalon"],
        110,
    )
    urls("api_testing", ["web.postman.co", "app.insomnia.rest", "hoppscotch.io", "usebruno.com"], 110)

    # --- Browsers (low priority fallback) ---
    products(
        "browsing",
        ["brave", "chrome", "chromium", "firefox", "mozilla firefox", "msedge", "microsoft edge",
         "safari", "opera", "vivaldi", "tor browser", "librewolf", "waterfox", "ungoogled"],
        55,
    )
    aumids("browsing", ["brave-browser", "google-chrome", "firefox", "microsoft-edge", "chromium"], 55)
    titles("browsing", [" - brave", " - google chrome", " - chromium", " - mozilla firefox", " - microsoft edge"], 45)

    # --- Web search (beats browsing) ---
    urls(
        "web_search",
        [
            "google.com/search",
            "www.google.com/search",
            "bing.com/search",
            "duckduckgo.com",
            "search.brave.com",
            "search.yahoo.com",
        ],
        125,
    )
    titles("web_search", [" - google search", "google search", " - bing", " - duckduckgo"], 118)

    # --- Official streaming ---
    urls(
        "media_streaming_official",
        [
            "netflix.com", "www.netflix.com", "disneyplus.com", "disney+", "hulu.com",
            "primevideo.com", "amazon.com/gp/video", "max.com", "hbomax.com", "peacocktv.com",
            "paramountplus.com", "apple.com/tv", "tv.apple.com", "play.google.com/movies",
            "youtube.com/tv", "youtu.be", "crunchyroll.com", "funimation.com", "hidive.com",
            "mubi.com", "criterionchannel.com", "shudder.com", "britbox.com", "acorntv.com",
            "discoveryplus.com", "espn.com", "espnplus.com", "dazn.com", "fubo.tv",
            "sling.com", "pluto.tv", "tubitv.com", "roku.com", "pbs.org", "bbc.co.uk/iplayer",
            "iplayer", "itv.com", "channel4.com", "my5.tv", "arte.tv", "raiplay.it",
            "mediasetplay", "joyn.de", "tvnow.de", "zdf.de", "ardmediathek.de",
            "hotstar.com", "sonyliv.com", "zee5.com", "voot.com", "mxplayer.in",
            "viki.com", "iq.com", "viki", "wetv.vip", "youku.com", "bilibili.tv",
            "bilibili.com", "niconico", "abema.tv", "tving.com", "wavve.com",
            "stan.com.au", "binge.com.au", "kayo", "neon.tv", "tvnz.co.nz",
            "crave.ca", "cbc.ca/gem", "ctv.ca", "globoplay", "clarovideo", "starplus.com",
            "sky.com", "nowtv.com", "canalplus.com", "salto.fr", "viaplay.com",
            "discovery+", "showtime.com", "starz.com", "mgm+", "amcplus.com",
            "kanopy.com", "hoopladigital.com", "library.kanopy",
        ],
        135,
    )
    titles(
        "media_streaming_official",
        [
            "netflix", "disney+", "disney plus", "prime video", "amazon prime video",
            "hulu", "hbo max", "max |", "peacock", "paramount+", "apple tv+", "crunchyroll",
            "funimation", "mubi", "bbc iplayer", "youtube music", # youtube watch handled below
        ],
        130,
    )
    products(
        "media_streaming_official",
        ["netflix", "disney+", "prime video", "hulu", "plex amp", "apple tv"],
        125,
    )

    # YouTube watch vs Shorts vs Studio split via more specific titles/urls
    urls("media_streaming_official", ["youtube.com/watch", "youtu.be/", "youtube.com/embed"], 132)
    urls("social_short", ["youtube.com/shorts", "youtube.com/hashtag/shorts"], 140)
    urls("content_creation", ["studio.youtube.com", "youtube.com/upload"], 145)
    titles("media_streaming_official", [" - youtube", "youtube - "], 128)

    # --- Unofficial / free streaming sites ---
    urls(
        "media_streaming_unofficial",
        [
            "fmovies", "soap2day", "sflix", "flixhq", "9anime", "aniwave", "gogoanime",
            "zoro.to", "hianime", "kickassanime", "animepahe", "123movies", "lookmovie",
            "movies2watch", "hdmovie2", "himovies", "m4uhd", "cineb", "braflix",
            "hydrahd", "pressplay", "sflix.to", "watchseries", "projectfreetv",
            "putlocker", "solarmovie", "gomovies", "yesmovies", "primewire",
            "streamlord", "vidstream", "embed.su", "vidsrc", "2embed", "superembed",
            "moviesjoy", "myflixer", "hurawatch", "sflix.se", "watchserieshd",
            "aniwatch", "aniworld", "bs.to", "kinoger", "kinox", "movie4k",
            "serienstream", "burning-series", "cinemathek", "streamkiste",
            "french-stream", "wiflix", "papystreaming", "voiranime", "vostfree",
            "animeflv", "jkanime", "tioanime", "monoschinos", "cuevana", "pelisplushd",
            "repelis", "gnula", "seriesyonkis", "megafilmeshd", "comandotorrents",
            "torrentgalaxy", "1337x", "rarbg", "yts.mx", "yts.", "eztv", "limetorrents",
            "nyaa.si", "animetosho", "rutracker", "thepiratebay", "tpb.", "magnet:?xt",
            "stremio", "torrentio", "mediafusion", "elfhosted", "aiostreams",
            "popcorn-time", "popcorntime", "webtorrent",
        ],
        138,
    )
    titles(
        "media_streaming_unofficial",
        ["fmovies", "soap2day", "9anime", "aniwave", "hianime", "braflix", "stremio", "popcorn time"],
        133,
    )
    products("media_streaming_unofficial", ["stremio", "popcorn-time", "webtorrent"], 130)

    # --- Local media ---
    products(
        "media_local",
        ["vlc", "mpv", "jellyfin", "plex", "emby", "kodi", "osmc", "infuse", "iina",
         "quicktime", "windows media player", "movies & tv", "celluloid", "smplayer",
         "potplayer", "mpc-hc", "mpc-be", "haruna", "clapper", "totem"],
        120,
    )
    aumids("media_local", ["vlc", "mpv", "org.jellyfin", "tv.plex"], 118)
    # MPRIS alone is a weak signal (browsers also use it). Prefer player identity /
    # URL / title rules; keep a low-priority fallback only.
    add("media_local", "focused_control_type", "equals", "mpris", 35)
    for player, slug, p in [
        # Browser MPRIS alone is not enough — Twitter/IG tabs also expose MPRIS.
        # Only dedicated players force a media category from automation_id.
        ("spotify", "music_listening", 155),
        ("vlc", "media_local", 150),
        ("mpv", "media_local", 150),
        ("jellyfin", "media_local", 150),
        ("plex", "media_local", 150),
        ("rhythmbox", "music_listening", 150),
        ("Lollypop", "music_listening", 150),
    ]:
        add(slug, "automation_id", "contains", player, p)

    # --- Music ---
    products(
        "music_listening",
        ["spotify", "apple music", "amazon music", "tidal", "deezer", "soundcloud",
         "youtube music", "pandora", "audacious", "rhythmbox", "clementine", "strawberry",
         "fooyin", "elisa", "amarok", "foobar2000", "musicbee", "aimp", "deadbeef",
         "nuclear", "nuclear music"],
        120,
    )
    urls(
        "music_listening",
        ["open.spotify.com", "music.apple.com", "music.amazon.com", "tidal.com", "deezer.com",
         "soundcloud.com", "music.youtube.com", "pandora.com", "bandcamp.com", "mixcloud.com",
         "audiomack.com", "radio.garden", "tunein.com"],
        125,
    )
    titles("music_listening", ["spotify", "apple music", "youtube music", "soundcloud", "tidal"], 118)

    # --- Live ---
    urls(
        "live_streaming",
        ["twitch.tv", "kick.com", "youtube.com/live", "afreecatv", "sooplive", "trovo.live",
         "facebook.com/gaming", "dlive.tv", "caffeine.tv", "rumble.com"],
        130,
    )
    products("live_streaming", ["twitch", "streamlabs", "obs studio", "obs64", "streamElements"], 100)
    titles("live_streaming", ["twitch", "kick.com", " - live - youtube"], 125)
    # OBS is creation more than watching
    products("content_creation", ["obs studio", "obs64", "streamlabs", "streamlabs desktop", "prism live"], 125)

    # --- Social short ---
    urls(
        "social_short",
        [
            "tiktok.com", "instagram.com/reels", "instagram.com/stories", "instagram.com/reel",
            "facebook.com/reel", "facebook.com/stories", "snapchat.com", "threads.net",
            "beReal.app", "bereal.com", "triller.co", "likee.video", "capcut.com",
            "lemon8", "youtube.com/shorts",
        ],
        140,
    )
    titles(
        "social_short",
        ["tiktok", "instagram reels", "instagram stories", "youtube shorts", "snapchat", "threads"],
        135,
    )
    products("social_short", ["tiktok", "instagram", "snapchat", "capcut"], 120)

    # --- Social long ---
    urls(
        "social_long",
        [
            "youtube.com", "youtu.be", "linkedin.com", "reddit.com", "old.reddit.com",
            "news.ycombinator.com", "lobste.rs", "medium.com", "substack.com", "ghost.org",
            "wordpress.com", "blogger.com", "tumblr.com", "pinterest.com", "quora.com",
            "stackexchange.com", "stackoverflow.com", "dev.to", "hashnode.com",
            "facebook.com", "fb.com", "x.com", "twitter.com", "nitter", "mastodon",
            "bsky.app", "bluesky", "truthsocial.com", "gab.com", "vk.com", "ok.ru",
            "weibo.com", "xiaohongshu.com", "douyin.com", "zhihu.com",
        ],
        100,
    )
    # Raise specificity so Shorts/streaming win over generic youtube.com above — already higher prio
    titles(
        "social_long",
        [
            "linkedin", "reddit", "hacker news", "substack", "medium.com", "stackoverflow",
            "twitter", " facebook", "mastodon", "bluesky",
        ],
        95,
    )
    # High-priority X / Instagram title matchers (must beat browsing@55).
    titles(
        "social_long",
        [
            " / x -",
            " / x",
            " on x:",
            "instagram -",
            "instagram",
            "x.com",
            "twitter.com",
        ],
        140,
    )
    products("social_long", ["linkedin", "reddit"], 90)

    # --- Messaging ---
    products(
        "social_messaging",
        ["whatsapp", "telegram", "signal", "messenger", "imessage", "messages", "element",
         "riot", "matrix", "threema", "viber", "line", "wechat", "kakao", "discord"],
        105,
    )
    urls(
        "social_messaging",
        ["web.whatsapp.com", "web.telegram.org", "signal.org", "messages.google.com",
         "messenger.com", "discord.com/channels", "app.element.io", "chat.google.com"],
        110,
    )

    # --- Content creation ---
    urls(
        "content_creation",
        [
            "studio.youtube.com", "creator.instagram.com", "ads.tiktok.com", "tiktok.com/creator",
            "business.facebook.com", "ads.twitter.com", "ads.linkedin.com", "canva.com",
            "capcut.com", "descript.com", "riverside.fm", "opus.pro", "veed.io", "kapwing.com",
            "invideo.io", "pictory.ai", "buffer.com", "hootsuite.com", "later.com",
            "sproutsocial.com", "publer.io", "metricool.com", "socialbee.com",
            "preview.app", "previewed.app",
        ],
        130,
    )
    products(
        "content_creation",
        ["canva", "capcut", "descript", "final cut", "davinci resolve", "premiere", "after effects"],
        110,
    )

    # --- Meetings / comms / email ---
    products(
        "meetings",
        ["zoom", "microsoft teams", "webex", "gotomeeting", "bluejeans", "whereby", "around",
         "loom", "jitsi", "ringcentral", "8x8", "skype", "facetime"],
        130,
    )
    urls(
        "meetings",
        ["zoom.us", "teams.microsoft.com", "meet.google.com", "meet.jit.si", "webex.com",
         "whereby.com", "around.co", "loom.com", "skype.com", "facetime.apple.com"],
        135,
    )
    titles("meetings", ["zoom meeting", "microsoft teams", "google meet", "meet.google", "webex"], 132)

    products(
        "communication",
        ["slack", "mattermost", "rocket.chat", "microsoft teams", "workplace", "twist", "fleep"],
        115,
    )
    urls(
        "communication",
        ["app.slack.com", "slack.com", "teams.microsoft.com", "mattermost", "discord.com",
         "chat.google.com", "workplace.com"],
        112,
    )
    # Discord often social — keep both; product Discord already in messaging at 105

    products(
        "email",
        ["outlook", "thunderbird", "mailspring", "evolution", "geary", "fairmail", "spark mail",
         "airmail", "mailbird", "em client", "windows mail", "apple mail"],
        110,
    )
    urls(
        "email",
        ["mail.google.com", "gmail.com", "outlook.live.com", "outlook.office.com", "outlook.office365.com",
         "mail.yahoo.com", "proton.me/mail", "protonmail.com", "mail.aol.com", "icloud.com/mail",
         "fastmail.com", "hey.com", "superhuman.com", "front.com", "missiveapp.com"],
        120,
    )
    titles("email", ["gmail", "inbox (", "outlook", "proton mail", "thunderbird"], 110)

    # --- Office suite ---
    products(
        "office_docs",
        ["microsoft word", "winword", "libreoffice writer", "writer", "pages", "abiword",
         "onlyoffice", "wps writer", "google docs"],
        100,
    )
    urls(
        "office_docs",
        ["docs.google.com", "word.cloud.microsoft", "office.com/launch/word", "notion.so",
         "coda.io", "dropbox.com/paper", "paper.dropbox.com", "quip.com", "craft.do",
         "bear.app", "ulysses.app", "ia writer", "scrivener"],
        120,
    )
    titles("office_docs", ["google docs", "microsoft word", ".docx", " - word"], 110)

    products(
        "spreadsheets",
        ["microsoft excel", "excel", "libreoffice calc", "calc", "numbers", "gnumeric", "onlyoffice"],
        100,
    )
    urls(
        "spreadsheets",
        ["sheets.google.com", "docs.google.com/spreadsheets", "excel.cloud.microsoft",
         "airtable.com", "rows.com", "smartsheet.com", "grid.is", "causal.app"],
        125,
    )
    titles("spreadsheets", ["google sheets", "microsoft excel", ".xlsx", " - excel"], 115)

    products(
        "presentations",
        ["powerpoint", "libreoffice impress", "keynote", "onlyoffice", "prezi", "beautiful.ai"],
        100,
    )
    urls(
        "presentations",
        ["slides.google.com", "docs.google.com/presentation", "powerpoint.cloud.microsoft",
         "prezi.com", "beautiful.ai", "pitch.com", "gamma.app", "tome.app", "slidebean.com"],
        120,
    )
    titles("presentations", ["google slides", "powerpoint", "keynote", ".pptx"], 110)

    products(
        "notes_wiki",
        ["notion", "obsidian", "logseq", "roam", "remnote", "craft", "bear", "joplin",
         "standard notes", "onenote", "evernote", "simplenote", "zettlr", "trilium",
         "anytype", "capacities", "reflect", "mem.ai"],
        100,
    )
    urls(
        "notes_wiki",
        ["notion.so", "notion.site", "obsidian.md", "logseq.com", "roamresearch.com",
         "confluence", "atlassian.net/wiki", "outline.wiki", "gitbook.com", "bookstack",
         "slab.com", "nuclino.com", "coda.io", "affine.pro", "app.heptabase.com"],
        115,
    )

    # --- BI / management / business ---
    products(
        "bi_analytics",
        ["power bi", "powerbi", "tableau", "qlik", "looker", "metabase", "superset",
         "redash", "mode analytics", "sisense", "domo", "thoughtspot", "hex", "observable"],
        125,
    )
    urls(
        "bi_analytics",
        [
            "app.powerbi.com", "powerbi.microsoft.com", "tableau.com", "public.tableau.com",
            "looker.com", "lookerstudio.google.com", "datastudio.google.com", "metabase",
            "superset", "mode.com", "hex.tech", "observablehq.com", "amplitude.com",
            "mixpanel.com", "analytics.google.com", "ads.google.com", "business.facebook.com/analytics",
            "hotjar.com", "fullstory.com", "heap.io", "posthog.com", "plausible.io",
            "matomo", "clarity.microsoft.com", "segment.com", "snowflake.com/worksheets",
            "app.databricks.com", "redash", "preset.io", "lightdash", "evidence.dev",
            "sigma computing", "sigmacomputing.com", "qlikcloud.com", "qlik.com",
            "domo.com", "sisense.com", "thoughtspot.com", "gooddata.com",
        ],
        130,
    )
    titles("bi_analytics", ["power bi", "tableau", "looker studio", "google analytics", "metabase"], 125)

    urls(
        "project_mgmt",
        [
            "atlassian.net", "jira", "linear.app", "asana.com", "trello.com", "monday.com",
            "clickup.com", "basecamp.com", "height.app", "shortcut.com", "clubhouse.io",
            "pivotaltracker.com", "azure.com/devops", "dev.azure.com", "youtrack",
            "wrike.com", "teamwork.com", "smartsheet.com", "airtable.com", "notion.so",
            "productboard.com", "aha.io", "prodpad.com", "miro.com", "figjam",
            "whimsical.com", "lucid.app", "lucidchart.com", "github.com/issues",
            "github.com/projects", "gitlab.com/-/boards", "plane.so", "openproject",
        ],
        110,
    )
    titles("project_mgmt", ["jira", "linear", "asana", "trello", "monday.com", "clickup", "azure devops"], 105)
    products("project_mgmt", ["jira", "trello"], 100)

    urls(
        "crm_sales",
        [
            "salesforce.com", "lightning.force.com", "hubspot.com", "pipedrive.com",
            "zoho.com/crm", "crm.zoho", "freshsales", "close.com", "copper.com",
            "attio.com", "affinity.co", "outreach.io", "salesloft.com", "gong.io",
            "chorus.ai", "apollo.io", "zoominfo.com", "linkedin.com/sales",
            "dynamics.microsoft.com", "dynamics365", "sugarcrm", "insightly.com",
        ],
        120,
    )
    products("crm_sales", ["salesforce", "hubspot", "pipedrive"], 115)
    titles("crm_sales", ["salesforce", "hubspot", "pipedrive", "dynamics 365"], 115)

    urls(
        "hr_people",
        [
            "workday.com", "bamboohr.com", "greenhouse.io", "lever.co", "ashbyhq.com",
            "jobvite.com", "icims.com", "successfactors", "adp.com", "gusto.com",
            "rippling.com", "deel.com", "remote.com", "personio.com", "hibob.com",
            "lattice.com", "15five.com", "cultureamp.com", "greenhouse", "lever",
            "linkedin.com/talent", "indeed.com", "glassdoor.com", "wellfound.com",
            "angel.co", "dover.com", "teamtailor.com", "pinpoint",
        ],
        115,
    )
    titles("hr_people", ["workday", "bamboohr", "greenhouse", "gusto", "rippling", "lattice"], 110)

    urls(
        "finance_accounting",
        [
            "quickbooks", "qbo.intuit.com", "xero.com", "freshbooks.com", "waveapps.com",
            "sage.com", "netsuite.com", "sap.com", "oracle.com/netsuite", "workday.com/finance",
            "expensify.com", "concur.sap", "brex.com", "ramp.com", "bill.com",
            "stripe.com/dashboard", "dashboard.stripe.com", "paypal.com/business",
            "square.com", "shopify.com/admin", "accounting", "bench.co", "pilot.com",
            "coupa.com", "tipalti.com", "bill.com", "melio.com", "mercury.com",
            "notebook.anaplan.com", "adaptive insights", "workday adaptive",
        ],
        120,
    )
    products("finance_accounting", ["quickbooks", "xero", "sage"], 110)
    titles("finance_accounting", ["quickbooks", "xero", "netsuite", "stripe dashboard", "expensify"], 115)

    urls(
        "legal_compliance",
        [
            "docusign.com", "hellosign.com", "dropboxsign.com", "adobe.com/sign",
            "ironcladapp.com", "contractbook.com", "lawgeex", "lexisnexis", "westlaw",
            "clio.com", "mycase.com", "practicepanther", "relativity", "everlaw",
            "vanta.com", "drata.com", "secureframe.com", "one trust", "onetrust.com",
            "osano.com", "cookiebot", "termly.io", "iubenda.com",
        ],
        115,
    )
    titles("legal_compliance", ["docusign", "helloSign", "adobe sign", "vanta", "drata", "westlaw"], 110)

    urls(
        "exec_management",
        [
            "lattice.com/okrs", "ally.io", "weekdone.com", "perdoo.com", "quantive.com",
            "workboard.com", "gtmhub", "profit.co", "betterworks.com", "15five.com",
            "boardable.com", "diligent.com", "onboardmeetings", "slideboard",
            "strategyzer.com", "miro.com/app", "whimsical.com", "figjam",
            "coda.io/docs", "notion.so", "roam", "visible.vc", "carta.com",
            "pulley.com", "angellist.com", "affinity.co", "harmonic.ai",
            "crunchbase.com", "pitchbook.com", "cbinsights.com",
        ],
        105,
    )
    titles("exec_management", ["okr", "board deck", "board meeting", "carta", "executive dashboard"], 100)

    urls(
        "customer_support",
        [
            "zendesk.com", "freshdesk.com", "intercom.com", "helpscout.com", "gorgias.com",
            "kustomer.com", "freshchat", "crisp.chat", "drift.com", "livechat.com",
            "front.com", "kayako.com", "happyfox.com", "service-now.com", "servicenow.com",
            "jira.service", "osTicket", "usedesk", "hubspot.com/service",
        ],
        115,
    )
    titles("customer_support", ["zendesk", "freshdesk", "intercom", "helpscout", "servicenow"], 110)

    # --- Design / media production ---
    products(
        "design_2d",
        ["figma", "sketch", "adobe xd", "photoshop", "illustrator", "indesign", "affinity",
         "affinity photo", "affinity designer", "affinity publisher", "inkscape", "gimp",
         "krita", "penpot", "photopea", "pixelmator", "procreate", "clip studio",
         "corelDRAW", "corel draw", "paint.net", "aseprite", "piskel"],
        115,
    )
    urls(
        "design_2d",
        ["figma.com", "penpot.app", "photopea.com", "canva.com", "adobe.com/products/photoshop",
         "creativecloud.adobe.com", "affinity.serif.com", "dribbble.com", "behance.net"],
        120,
    )
    titles("design_2d", ["figma", "photoshop", "illustrator", "affinity", "penpot"], 112)

    products(
        "vfx_3d",
        ["blender", "houdini", "nuke", "maya", "3ds max", "cinema 4d", "c4d", "zbrush",
         "substance", "marvelous designer", "unreal", "ue5", "unity", "godot",
         "cascadeur", "embergen", "realflow", "katana", "mari", "mudbox", "modo",
         "lightwave", "rhino", "grasshopper", "solidworks", "fusion 360", "onshape",
         "freecad", "openscad", "sketchup", "revit", "autocad", "archicad"],
        120,
    )
    titles("vfx_3d", ["blender", "houdini", "autodesk maya", "cinema 4d", "unreal engine", "unity"], 115)

    products(
        "video_editing",
        ["premiere", "after effects", "davinci", "resolve", "final cut", "avid media",
         "vegas pro", "kdenlive", "shotcut", "openshot", "lightworks", "hitfilm",
         "capcut", "filmora", "camtasia", "screenflow", "descript"],
        115,
    )
    titles("video_editing", ["premiere pro", "after effects", "davinci resolve", "final cut", "kdenlive"], 110)

    products(
        "audio_production",
        ["ableton", "fl studio", "logic pro", "pro tools", "reaper", "cubase", "studio one",
         "bitwig", "garageband", "audacity", "ardour", "lmms", "cakewalk", "reason",
         " Presonus", "izotope", "rx ", "ozone"],
        115,
    )
    titles("audio_production", ["ableton live", "fl studio", "logic pro", "pro tools", "reaper", "audacity"], 110)

    # --- Education / research / health ---
    urls(
        "education",
        [
            "coursera.org", "udemy.com", "edx.org", "khanacademy.org", "udacity.com",
            "skillshare.com", "pluralsight.com", "linkedin.com/learning", "codecademy.com",
            "freecodecamp.org", "leetcode.com", "hackerrank.com", "codewars.com",
            "brilliant.org", "duolingo.com", "busuu.com", "babbel.com", "ankiweb.net",
            "quizlet.com", "canvas.instructure.com", "blackboard.com", "moodle",
            "classroom.google.com", "schoology.com", "teachable.com", "thinkific.com",
            "masterclass.com", "domestika.org", "superpeer.com", "maven.com",
        ],
        110,
    )
    titles("education", ["coursera", "udemy", "khan academy", "leetcode", "duolingo", "canvas"], 105)

    urls(
        "research",
        [
            "scholar.google.com", "pubmed.ncbi", "arxiv.org", "semanticscholar.org",
            "researchgate.net", "academia.edu", "jstor.org", "sciencedirect.com",
            "springer.com", "nature.com", "science.org", "ieee.org", "acm.org",
            "ssrn.com", "zotero.org", "mendeley.com", "endnote", "papersapp.com",
            "readcube.com", "connectedpapers.com", "elicit.org", "consensus.app",
            "scite.ai", "researchrabbit.ai", "lens.org", "webofknowledge",
            "webofscience", "scopus.com", "orcid.org",
        ],
        115,
    )
    products("research", ["zotero", "mendeley", "endnote", "papers"], 110)
    titles("research", ["arxiv", "pubmed", "google scholar", "zotero", "mendeley"], 110)

    urls(
        "healthcare",
        [
            "epic.com", "mychart", "cerner", "athenahealth", "allscripts", "eclinicalworks",
            "doximity.com", "practito", "teladoc", "amwell", "mdlive", "zocdoc.com",
            "healthline.com", "webmd.com", "mayoclinic.org", "drugs.com", "uptodate.com",
            "epocrates", "figure1", "doximity",
        ],
        110,
    )
    titles("healthcare", ["mychart", "epic ", "uptodate", "webmd", "teladoc"], 105)

    # --- Reading / files / calendar ---
    products(
        "reading",
        ["evince", "okular", "zathura", "atril", "foxit", "adobe acrobat", "acrobat reader",
         "sumatra", "preview", "calibre", "foliate", "books", "kindle", "kobo", "moon+ reader"],
        90,
    )
    urls(
        "reading",
        ["wikipedia.org", "wikiwand.com", "developer.mozilla.org", "docs.python.org",
         "readthedocs.io", "gitbook.io", "notion.site", "are.na", "pocket.com",
         "getpocket.com", "instapaper.com", "omnivore.app", "reader.tt-rss", "newsblur.com",
         "feedly.com", "inoreader.com", "goodreads.com", "literal.club", "thestorygraph.com",
         "archive.org", "gutenberg.org", "libgen", "sci-hub", "annas-archive"],
        85,
    )
    titles("reading", [".pdf", "wikipedia", "mozilla developer", "calibre", "kindle"], 70)

    products(
        "file_management",
        ["nautilus", "dolphin", "thunar", "pcmanfm", "nemo", "caja", "files", "finder",
         "explorer", "double commander", "total commander", "directory opus", "ranger",
         "nnn", "lf", "yazi", "far manager"],
        100,
    )
    aumids("file_management", ["Nautilus", "org.gnome.Nautilus", "dolphin", "thunar"], 100)
    urls(
        "file_management",
        ["drive.google.com", "dropbox.com", "onedrive.live.com", "onedrive.microsoft.com",
         "box.com", "mega.nz", "icloud.com/iclouddrive", "nextcloud", "pcloud.com",
         "sync.com", "tresorit.com", "proton.me/drive"],
        105,
    )
    titles("file_management", ["google drive", "dropbox", "onedrive", "file manager"], 95)

    products(
        "calendar_tasks",
        ["gnome-calendar", "kalendar", "outlook", "fantastical", "things", "todoist",
         "ticktick", "microsoft to do", "omnifocus", "reminders", "calendar"],
        90,
    )
    urls(
        "calendar_tasks",
        ["calendar.google.com", "outlook.office.com/calendar", "todoist.com", "ticktick.com",
         "app.todoist.com", "tasks.office.com", "notion.so", "any.do", "clickup.com",
         "linear.app", "height.app", "reminders"],
        100,
    )
    titles("calendar_tasks", ["google calendar", "todoist", "ticktick", "microsoft to do"], 95)

    # --- Personal life ---
    urls(
        "banking_finance_personal",
        [
            "chase.com", "bankofamerica.com", "wellsfargo.com", "citi.com", "capitalone.com",
            "americanexpress.com", "discover.com", "ally.com", "sofi.com", "revolut.com",
            "wise.com", "paypal.com", "venmo.com", "cash.app", "robinhood.com",
            "fidelity.com", "vanguard.com", "schwab.com", "etrade.com", "coinbase.com",
            "binance.com", "kraken.com", "blockchain.com", "mint.intuit.com",
            "ynab.com", "monarchmoney.com", "empower.com", "personalcapital",
            "stripe.com", "squareup.com", "klarna.com", "afterpay.com",
        ],
        110,
    )
    titles("banking_finance_personal", ["chase", "bank of america", "robinhood", "coinbase", "revolut", "wise"], 100)

    urls(
        "shopping",
        [
            "amazon.com", "amazon.", "ebay.com", "etsy.com", "walmart.com", "target.com",
            "aliexpress.com", "alibaba.com", "shopify.com", "bestbuy.com", "newegg.com",
            "costco.com", "ikea.com", "homedepot.com", "lowes.com", "wayfair.com",
            "zalando", "asos.com", "shein.com", "temu.com", "wish.com", "mercari.com",
            "craigslist.org", "facebook.com/marketplace", "offerup.com", "poshmark.com",
            "stockx.com", "goat.com", "nike.com", "adidas.com", "apple.com/shop",
        ],
        90,
    )
    titles("shopping", ["amazon.", "ebay", "etsy", "walmart", "aliexpress", "marketplace"], 85)

    urls(
        "travel_maps",
        [
            "maps.google.com", "google.com/maps", "apple.com/maps", "bing.com/maps",
            "openstreetmap.org", "waze.com", "citymapper.com", "transitapp.com",
            "booking.com", "airbnb.com", "expedia.com", "kayak.com", "skyscanner.",
            "tripadvisor.com", "hotels.com", "agoda.com", "vrbo.com", "uber.com",
            "lyft.com", "bolt.eu", "trainline.com", "rome2rio.com", "flightradar24.com",
            "tripadvisor", "marriott.com", "hilton.com", "delta.com", "united.com",
            "aa.com", "ryanair.com", "easyjet.com", "turkishairlines.com",
        ],
        100,
    )
    products("travel_maps", ["maps", "waze", "organic maps", "osmand"], 90)
    titles("travel_maps", ["google maps", "booking.com", "airbnb", "uber", "waze"], 95)

    products(
        "gaming",
        [
            "steam", "steamwebhelper", "lutris", "heroic", "legendary", "rare", "bottles",
            "epic games", "gog galaxy", "origin", "ea app", "battle.net", "battlenet",
            "riot client", "league of legends", "valorant", "riot games",
            "xbox", "gamingservices", "game bar", "playstation", "ps remote play",
            "nvidia geForce", "geforce now", "moonlight", "sunshine", "parsec",
            "retroarch", "dolphin-emu", "pcsx2", "rpcs3", "duckstation", "ppsspp",
            "yuzu", "ryujinx", "cemu", "citra", "melonds", "mgba", "snes9x",
            "minecraft", "roblox", "fortnite", "osu!", "osu", "wine", "proton",
            "itch", "itch.io", "gog", "ubisoft connect", "uplay", "rockstar games",
            "battlestate", "escape from tarkov", "path of exile", "wow", "world of warcraft",
            "final fantasy xiv", "ffxiv", "guild wars", "runelite", "osrs",
            "counter-strike", "cs2", "dota", "team fortress", "apex legends",
            "overwatch", "destiny 2", "warframe", "elden ring", "baldur",
            "stardew", "terraria", "factorio", "rimworld", "valheim", "palworld",
            "genshin", "honkai", "leagueclient",
        ],
        100,
    )
    aumids(
        "gaming",
        ["steam", "lutris", "heroic", "com.valvesoftware.Steam", "minecraft", "roblox"],
        100,
    )
    urls(
        "gaming",
        [
            "store.steampowered.com", "steamcommunity.com", "steamdb.info",
            "epicgames.com", "store.epicgames.com", "gog.com", "itch.io",
            "roblox.com", "minecraft.net", "minecraft.fandom.com",
            "xbox.com", "xboxlive.com", "playstation.com", "nintendo.com",
            "geforcenow.com", "nvidia.com/en-us/geforce-now", "parsec.app",
            "battle.net", "blizzard.com", "riotgames.com", "playvalorant.com",
            "leagueoflegends.com", "op.gg", "u.gg", "porofessor.gg",
            "pathofexile.com", "twitch.tv", "kick.com",
            "escapefromtarkov.com", "warframe.com", "overwolf.com",
            "curseforge.com", "modrinth.com", "nexusmods.com",
            "speedrun.com", "howlongtobeat.com", "protondb.com", "areweanticheatyet.com",
            "fortnitetracker.com", "tracker.gg", "faceit.com", "esea.net",
            "chess.com", "lichess.org", "crazygames.com", "poki.com", "miniclip.com",
            "store.steampowered.com/app", "ea.com", "ubisoft.com", "rockstargames.com",
        ],
        95,
    )
    titles(
        "gaming",
        [
            "steam", "epic games", "battle.net", "geforce now", "lutris", "heroic games",
            "counter-strike", "dota 2", "league of legends", "valorant", "minecraft",
            "roblox", "fortnite", "path of exile", "world of warcraft", "osu!",
        ],
        95,
    )
    companies(
        "gaming",
        ["Valve", "Epic Games", "Riot Games", "Blizzard", "Electronic Arts", "Ubisoft",
         "Rockstar Games", "Nintendo", "Sony Interactive", "Xbox Game Studios"],
        85,
    )
    paths(
        "gaming",
        ["steamapps", "Steam/steamapps", "lutris", "Heroic/Games", "Epic Games",
         ".local/share/Steam", "riot-client", "Battle.net"],
        95,
    )

    products(
        "system_settings",
        ["gnome-control-center", "systemsettings", "settings", "xfce4-settings",
         "software", "gnome-software", "discover", "synaptic", "pamac", "octopi",
         "app store", "microsoft store", "winget", "chocolatey", "homebrew"],
        70,
    )
    titles("system_settings", ["settings", "system settings", "software center", "app store", "gnome software"], 60)

    products(
        "ai_chat_general",
        ["chatgpt", "claude", "gemini", "perplexity", "poe", "character.ai", "grok"],
        100,
    )
    urls(
        "ai_chat_general",
        [
            "chat.openai.com", "chatgpt.com", "claude.ai", "gemini.google.com", "bard.google.com",
            "perplexity.ai", "poe.com", "character.ai", "grok.x.ai", "copilot.microsoft.com",
            "bing.com/chat", "you.com", "phind.com", "huggingface.co/chat", "pi.ai",
            "meta.ai", "llama", "mistral.ai/chat", "chat.mistral.ai", "together.ai",
            "openrouter.ai", "lmstudio", "ollama", "chatgpt",
        ],
        125,
    )
    titles("ai_chat_general", ["chatgpt", "claude", "gemini", "perplexity", "microsoft copilot"], 120)

    # Extra breadth: more domains / apps across categories to push past 1000 unique rules
    extra_official = [
        "paramountplus", "skyShowtime", "viaplay", "cda.pl", "player.pl", "tvp.pl",
        "player.mtg", "discoveryplus", "adultswim.com", "cartoonnetwork", "nick.com",
        "mlb.tv", "nhl.tv", "nba.com", "f1tv", "motogp", "ufc fight pass",
        "curiositystream.com", "nebula.tv", "dropout.tv", "vrv.co", "hidive",
        "shahid.mbc", "osnplus", "watchit", "anghami",
    ]
    urls("media_streaming_official", extra_official, 130)

    extra_unofficial = [
        "sflix.to", "flixhq.to", "fmoviesz", "movie-web", "vidbinge", "ripperoni",
        "cinehub", "netmirror", "m-team", "bitsearch", "bitspyder", "ext.to",
        "torlock", "glodls", "torrentdownloads", "zooqle", "skytorrents",
        "animekai", "hianime.to", "aniwatch.to", "miruro", "sudatchi", "allmanga",
        "mangadex.org", "comick.io", "batoto", "manganato", "asurascans",
        "webtoons.com", "tapas.io", "tappytoon.com", "lezhin.com",
    ]
    urls("media_streaming_unofficial", extra_unofficial, 136)

    extra_social = [
        "truth.social", "gettr.com", "mewe.com", "myspace.com", "nextdoor.com",
        "strava.com", "goodreads.com", "letterboxd.com", "untappd.com", "vsco.co",
        "flickr.com", "500px.com", "deviantart.com", "artstation.com", "cara.app",
        "patreon.com", "ko-fi.com", "buymeacoffee.com", "onlyfans.com", "fanhouse.app",
        "substack.com", "beehiiv.com", "ghost.io", "convertkit.com", "buttondown.email",
    ]
    urls("social_long", extra_social, 98)

    extra_short = [
        "snackvideo", "josh app", "chingari", "moj app", "sharechat", "roposo",
        "likee", "triller", "byte", "vine archive", "clash",
    ]
    urls("social_short", extra_short, 130)

    extra_office = [
        "wordonline", "office.com", "microsoft365.com", "workspace.google.com",
        "admin.google.com", "drive.google.com", "calendar.google.com",
        "zoho.com/writer", "zoho.com/sheet", "zoho.com/show", "onlyoffice.com",
        "cryptpad.fr", "collaboraonline", "nextcloud",
    ]
    urls("office_docs", extra_office, 108)

    extra_bi = [
        "app.hex.tech", "modeanalytics", "periscope", "sisense", "domo",
        "chartio", "klipfolio", "geckoboard", "databox", "whatagraph",
        "supermetrics", "funnel.io", "adverity", "segment", "rudderstack",
        "fivetran.com", "airbyte.com", "dbt.com", "getdbt.com", "census.dev",
        "hightouch.com", "preficture", "census",
    ]
    urls("bi_analytics", extra_bi, 125)

    extra_mgmt = [
        "okrs", "ally.io", "betterworks", "15five", "culture amp", "glint",
        "peakon", "officevibe", "tinypulse", "bonusly", "lattice reviews",
        "sequoia", "a16z", "notion hq", "levels.fyi",
    ]
    urls("exec_management", extra_mgmt, 100)

    extra_dev = [
        "github.com", "gitlab.com", "bitbucket.org", "sourceforge.net", "codeberg.org",
        "gitea", "gogs", "sr.ht", "pagure.io", "launchpad.net", "npmjs.com",
        "pypi.org", "crates.io", "rubygems.org", "nuget.org", "maven.org",
        "hub.docker.com", "play.google.com/console", "appstoreconnect.apple.com",
        "developer.apple.com", "console.firebase.google.com", "sentry.io",
        "bugsnag.com", "rollbar.com", "logrocket.com", "datadoghq.com",
    ]
    # github browsing often project work — project_mgmt already has issues/projects;
    # general github -> devops-ish coding collab
    urls("devops", [d for d in extra_dev if "github.com" not in d or d != "github.com"], 95)
    urls("project_mgmt", ["github.com", "gitlab.com", "bitbucket.org"], 75)

    extra_comms = [
        "keybase.io", "wire.com", "session.org", "briarproject", "status.im",
        "icq.com", "skype.com", "hangouts.google.com", "duo.google.com",
    ]
    urls("social_messaging", extra_comms, 100)

    extra_edu = [
        "brilliant.org", " Brilliant", "ankiweb", "quizlet", "chegg.com", "coursehero.com",
        "studocu.com", "brainly.com", "photomath.com", "wolframalpha.com",
        "symbolab.com", "desmos.com", "geogebra.org", "phet.colorado.edu",
    ]
    urls("education", extra_edu, 105)

    # Many Windows/Linux product aliases
    for name, slug, p in [
        ("WindowsTerminal", "terminal", 100),
        ("Windows Terminal", "terminal", 100),
        ("wt.exe", "terminal", 90),
        ("Code - Insiders", "code_editing", 95),
        ("code-insiders", "code_editing", 95),
        ("devenv", "code_editing", 90),
        ("MsMpEng", "system_settings", 40),  # skip noisy? still a rule
        ("explorer.exe", "file_management", 80),
        ("SearchHost", "system_settings", 50),
        ("ApplicationFrameHost", "system_settings", 30),
    ]:
        products(slug, [name], p)

    # Company-based rules for publishers
    for company, slug, p in [
        ("Microsoft Corporation", "office_docs", 40),  # too broad — skip high
        ("JetBrains", "code_editing", 85),
        ("Adobe Inc", "design_2d", 70),
        ("Adobe Systems", "design_2d", 70),
        ("Autodesk", "vfx_3d", 90),
        ("The Document Foundation", "office_docs", 80),
        ("Brave Software", "browsing", 50),
        ("Mozilla", "browsing", 50),
        ("Google LLC", "browsing", 30),
        ("Spotify AB", "music_listening", 110),
        ("Valve", "gaming", 90),
        ("Epic Games", "gaming", 90),
        ("Discord Inc", "social_messaging", 100),
        ("Slack Technologies", "communication", 110),
        ("Zoom Video", "meetings", 120),
        ("Figma", "design_2d", 120),
        ("Notion Labs", "notes_wiki", 100),
        ("Obsidian", "notes_wiki", 100),
        ("VideoLAN", "media_local", 115),
        ("Plex", "media_local", 110),
        ("Jellyfin", "media_local", 110),
    ]:
        companies(slug, [company], p)

    # Executable path heuristics
    for needle, slug, p in [
        ("/usr/bin/vlc", "media_local", 120),
        ("/usr/bin/mpv", "media_local", 120),
        ("/usr/bin/spotify", "music_listening", 120),
        ("/opt/google/chrome", "browsing", 50),
        ("/usr/lib64/firefox", "browsing", 50),
        ("/opt/brave.com", "browsing", 50),
        ("JetBrains", "code_editing", 85),
        ("Microsoft VS Code", "code_editing", 90),
        ("cursor-bin", "ai_coding", 120),
        ("/usr/bin/gimp", "design_2d", 110),
        ("/usr/bin/inkscape", "design_2d", 110),
        ("/usr/bin/blender", "vfx_3d", 120),
        ("steamapps", "gaming", 95),
        ("lutris", "gaming", 95),
    ]:
        paths(slug, [needle], p)


HEADER = """-- Sessions become category+context merges; discrete verbs stay as raw events.
-- Drop action spans. Add category taxonomy + user/LLM-editable activity rules.
-- Seed rules are generated by scripts/generate_activity_seed.py (1000+ rules).

CREATE TABLE IF NOT EXISTS category (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    slug TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    description TEXT,
    occupation_tags TEXT NOT NULL DEFAULT '[]',
    source TEXT NOT NULL DEFAULT 'seed',
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS activity_rule (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    category_id INTEGER NOT NULL,
    match_field TEXT NOT NULL,
    match_op TEXT NOT NULL,
    pattern TEXT NOT NULL,
    priority INTEGER NOT NULL DEFAULT 100,
    enabled INTEGER NOT NULL DEFAULT 1,
    source TEXT NOT NULL DEFAULT 'seed',
    notes TEXT,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (category_id) REFERENCES category (id),
    UNIQUE (match_field, match_op, pattern)
);

CREATE INDEX IF NOT EXISTS activity_rule_category_idx ON activity_rule (category_id);
CREATE INDEX IF NOT EXISTS activity_rule_priority_idx ON activity_rule (priority DESC);

ALTER TABLE session ADD COLUMN category_id INTEGER REFERENCES category (id);
ALTER TABLE session ADD COLUMN context_key TEXT;
ALTER TABLE session ADD COLUMN app_id INTEGER REFERENCES app (id);

CREATE INDEX IF NOT EXISTS session_category_idx ON session (category_id);
CREATE INDEX IF NOT EXISTS session_context_idx ON session (context_key);

DROP INDEX IF EXISTS event_action_idx;
DROP INDEX IF EXISTS action_session_idx;
UPDATE event SET action_id = NULL;
ALTER TABLE event DROP COLUMN action_id;
DROP TABLE IF EXISTS action;

"""


def sql_escape(s: str) -> str:
    return s.replace("'", "''")


def emit() -> str:
    build()
    lines = [HEADER]
    lines.append("INSERT OR IGNORE INTO category (slug, name, description, occupation_tags, source) VALUES")
    cat_rows = []
    for slug, name, desc, tags in CATEGORIES:
        cat_rows.append(
            f"('{sql_escape(slug)}', '{sql_escape(name)}', '{sql_escape(desc)}', '{sql_escape(tags)}', 'seed')"
        )
    lines.append(",\n".join(cat_rows) + ";\n")

    # Batch inserts of ~80 rules for readability
    items = sorted(rules.items(), key=lambda kv: (kv[1][0], -kv[1][1], kv[0][0], kv[0][2]))
    batch: list[str] = []
    for (field, op, _pat_l), (slug, priority) in items:
        # recover original pattern casing from key — we stored lowercased; use the key pattern
        pattern = _pat_l  # lowercase is fine for matching (case-insensitive)
        row = (
            f"((SELECT id FROM category WHERE slug='{sql_escape(slug)}'), "
            f"'{sql_escape(field)}', '{sql_escape(op)}', '{sql_escape(pattern)}', {priority}, 'seed')"
        )
        batch.append(row)
        if len(batch) >= 80:
            lines.append(
                "INSERT OR IGNORE INTO activity_rule "
                "(category_id, match_field, match_op, pattern, priority, source) VALUES\n"
                + ",\n".join(batch)
                + ";\n"
            )
            batch = []
    if batch:
        lines.append(
            "INSERT OR IGNORE INTO activity_rule "
            "(category_id, match_field, match_op, pattern, priority, source) VALUES\n"
            + ",\n".join(batch)
            + ";\n"
        )

    return "\n".join(lines), len(CATEGORIES), len(rules)


def main() -> None:
    sql, n_cats, n_rules = emit()
    OUT.write_text(sql)
    print(f"Wrote {OUT}")
    print(f"categories={n_cats} rules={n_rules}")
    if n_rules < 1000:
        raise SystemExit(f"Need >=1000 rules, got {n_rules}")


if __name__ == "__main__":
    main()
