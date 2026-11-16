//! Port of `lib/core/constants/api_constants.dart`.

pub const BASE_URL: &str = "https://pi.tgclab.com";
pub const METADATA_CDN_URL: &str = "https://picf.tgclab.com";
pub const MEDIA_CDN_URL: &str = "https://alicdn.tgclab.com";
pub const UPLOAD_URL: &str = "https://alifn.tgclab.com/file";
pub const AUDIO_CDN_URL: &str = "https://alicdn.pallo.cn";

pub const SIGN_IN_JWT: &str = "/user/sign-in-jwt";
pub const SOCIAL_SIGN_UP_JWT: &str = "/user/social/sign-up-jwt";
pub const SPLASH_LOGIN: &str = "/user/v2/splash-login";
pub const RELOAD_INFO: &str = "/user/v2/reload/info";
pub const SEND_SIGNUP_CODE: &str = "/user/v2/send-signup-code";
pub const VERIFY_CODE: &str = "/user/v2/verify-code";
pub const CREATE_EMAIL_ACCOUNT: &str = "/user/v2/create-email-account";
pub const UNREGISTER: &str = "/user/unregister";
pub const NICKNAME_CHANGE: &str = "/user/nickname/change";
pub const CATEGORY_BY_COUNTRY: &str = "/category/category-by-country";
pub const EXIST_USERNAME: &str = "/user/exist-username";

pub const SUBJECT_CREATE: &str = "/user/subject/create";
pub const SUBJECT_LIST: &str = "/user/subject/list";
pub const STUDY_START: &str = "/study/start";
pub const STUDY_STOP: &str = "/study/stop";
pub const STUDY_BREAK: &str = "/study/study-plan/rest";
pub const TIME_SYNC: &str = "/time/";

pub const GROUP_MEMBERS: &str = "/logs/group/members/v2";
pub const GROUP_SHAKE: &str = "/group/push/shake";
pub const GROUP_CHAT_MESSAGES: &str = "/chat/group/messages";
pub const GROUP_INFO_EDIT: &str = "/group/info/edit";

pub const USER_AGENT: &str = "Dart/3.11 (dart:io)";
pub const JWT_PREFIX: &str = "JWT ";
pub const DEFAULT_COUNTRY_ID: i32 = 23;
pub const DEFAULT_VERSION: i64 = 810041;
pub const DEFAULT_LANGUAGE: &str = "pt";
pub const DEFAULT_TIMEZONE: &str = "America/Sao_Paulo";
pub const DEFAULT_DEVICE_MODEL: &str = "Desktop";

pub fn default_device_type() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        "WIN"
    }
    #[cfg(target_os = "macos")]
    {
        "MAC"
    }
    #[cfg(target_os = "linux")]
    {
        "LINUX"
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        "WIN"
    }
}
