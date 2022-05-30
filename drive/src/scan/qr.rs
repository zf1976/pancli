use std::fmt::Debug;
use crate::error::QrCodeScannerError;
use crate::models::{auth, gen, query, suc, CkForm};
use crate::scan::QrCodeScanner;
use anyhow::anyhow;
use reqwest::blocking::Response;
use std::time;

// generator qrcode
const GENERATOR_QRCODE_API: &str = "https://passport.aliyundrive.com/newlogin/qrcode/generate.do?appName=aliyun_drive&fromSite=52&appEntrance=web&lang=zh_CN";
// query scanner result (include mobile token)
const QUERY_API: &str = "https://passport.aliyundrive.com/newlogin/qrcode/query.do?appName=aliyun_drive&fromSite=52&_bx-v=2.0.31";
// get session id
const SESSION_ID_API: &str = "https://auth.aliyundrive.com/v2/oauth/authorize?client_id=25dzX3vbYqktVxyX&redirect_uri=https%3A%2F%2Fwww.aliyundrive.com%2Fsign%2Fcallback&response_type=code&login_type=custom&state=%7B%22origin%22%3A%22https%3A%2F%2Fwww.aliyundrive.com%22%7D#/nestedlogin?keepLogin=false&hidePhoneCode=true&ad__pass__q__rememberLogin=true&ad__pass__q__rememberLoginDefaultValue=true&ad__pass__q__forgotPassword=true&ad__pass__q__licenseMargin=true&ad__pass__q__loginType=normal";
// scan scan result（include authorization code）
const TOKEN_LOGIN_API: &str = "https://auth.aliyundrive.com/v2/oauth/token_login";
// get web side token
const GET_WEB_TOKEN_API: &str = "https://api.aliyundrive.com/token/get";

const UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/100.0.4896.127 Safari/537.36";
const SESSION_ID_KEY: &str = "SESSIONID";
const TIMEOUT_SECS: u8 = 30;
const CONN_TIMEOUT_SECS: u8 = 10;

pub struct LoginQrCodeScanner {
    session_id: String,
    client: reqwest::blocking::Client,
}

impl LoginQrCodeScanner {
    pub fn new() -> Self {
        let session_id = match LoginQrCodeScanner::init_session() {
            Ok(session_id) => session_id,
            Err(e) => {
                panic!("get session id error: {}", e)
            }
        };
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(time::Duration::from_secs(CONN_TIMEOUT_SECS as u64))
            .timeout(time::Duration::from_secs(TIMEOUT_SECS as u64))
            .build().unwrap();
        Self { session_id, client }
    }

    // initialize session
    pub fn init_session() -> anyhow::Result<String> {
        let client = reqwest::blocking::Client::builder()
            .user_agent(UA)
            .connect_timeout(time::Duration::from_secs(CONN_TIMEOUT_SECS as u64))
            .timeout(time::Duration::from_secs(TIMEOUT_SECS as u64))
            .build()?;
        let resp = client.get(SESSION_ID_API).send()?;
        if resp.status().is_success() {
            for cookie in resp.cookies() {
                if cookie.name() == SESSION_ID_KEY {
                    return Ok(String::from(cookie.value()));
                }
            }
            return Err(anyhow!("Failed to get session id."));
        }
        let msg = ResponseHandler::response_error_msg_handler(resp);
        Err(anyhow!(msg))
    }
}

impl QrCodeScanner for LoginQrCodeScanner {
    fn generator(&self) -> crate::ScanResult<gen::GeneratorQrCodeResult> {
        let resp = self.client.get(GENERATOR_QRCODE_API).send()?;
        ResponseHandler::response_handler::<gen::GeneratorQrCodeResult>(resp)
    }

    fn query(&self, from: &impl CkForm) -> crate::ScanResult<query::QueryQrCodeResult> {
        log::debug!("request ck form: {:#?}", from);
        let resp = self.client.post(QUERY_API).form(&from.map_form()).send()?;
        ResponseHandler::response_handler::<query::QueryQrCodeResult>(resp)
    }

    fn token_login(&self, token: auth::Token) -> crate::ScanResult<suc::GotoResult> {
        let resp = self
            .client
            .post(TOKEN_LOGIN_API)
            .header(
                reqwest::header::COOKIE,
                format!("SESSIONID={}", &self.session_id),
            )
            .json(&token)
            .send()?;
        ResponseHandler::response_handler::<suc::GotoResult>(resp)
    }

    fn get_token(&self, auth: auth::AuthorizationCode) -> crate::ScanResult<suc::WebLoginResult> {
        let resp = self
            .client
            .post(GET_WEB_TOKEN_API)
            .header(
                reqwest::header::COOKIE,
                format!("SESSIONID={}", &self.session_id),
            )
            .json(&auth)
            .send()?;
        ResponseHandler::response_handler::<suc::WebLoginResult>(resp)
    }
}

struct ResponseHandler;

impl ResponseHandler {
    #[allow(dead_code)]
    fn response_unit_handler(resp: Response) -> crate::ScanResult<()> {
        if resp.status().is_success() {
            return Ok(());
        }
        let msg = ResponseHandler::response_error_msg_handler(resp);
        Err(QrCodeScannerError::from(msg))
    }

    fn response_handler<T: serde::de::DeserializeOwned + Debug>(resp: Response) -> crate::ScanResult<T> {
        if resp.status().is_success() {
            let result = resp.json::<T>()?;
            log::debug!("response result: {:#?}", &result);
            return Ok(result);
        }
        let msg = ResponseHandler::response_error_msg_handler(resp);
        Err(QrCodeScannerError::from(msg))
    }

    fn response_error_msg_handler(resp: Response) -> String {
        let msg = resp
            .text()
            .unwrap_or(String::from("An error occurred while extracting the body."));
        log::debug!(
            "defined in file: {}, defined on line: {}\nmessage: {:?}",
            file!(),
            line!(),
            &msg
        );
        msg
    }
}
