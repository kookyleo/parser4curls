use nom::{
    branch::alt,
    bytes::complete::{tag, tag_no_case, take, take_until, take_while},
    character::complete::multispace0,
    combinator::opt,
    multi::separated_list0,
    sequence::{delimited, tuple},
};
use nom::{AsChar, IResult, InputTakeAtPosition};
use std::collections::HashMap;
use std::fmt::Debug;

/// curl <url> [options...]
#[derive(Debug, PartialEq)]
pub struct Curl<'a> {
    pub url: &'a str,
    pub options_header_cookies: HashMap<&'a str, &'a str>,
    pub options_headers_more: HashMap<&'a str, &'a str>,
    pub options_data_raw: &'a str,
    pub options_more: HashMap<&'a str, &'a str>,
}

#[allow(dead_code)]
fn key(input: &str) -> IResult<&str, &str> {
    input.split_at_position_complete(|item| {
        !(item.is_alphanum() || item.as_char() == '-' || item.as_char() == '_')
    })
}

#[allow(dead_code)]
fn cookie_pairs(input: &str) -> IResult<&str, HashMap<&str, &str>> {
    let (input, cookies) = separated_list0(
        tag(";"),
        tuple((
            multispace0,
            key,
            tag("="),
            take_while(|ch| ch != '\'' && ch != ';'),
            multispace0,
        )),
    )(input)?;
    Ok((input, cookies.into_iter().map(|c| (c.1, c.3)).collect()))
}

#[allow(dead_code)]
fn options_header_cookie(input: &str) -> IResult<&str, HashMap<&str, &str>> {
    let (input, (_, _, cookies, _)) = tuple((
        alt((tag("-H\x20"), tag("--header\x20"))),
        tag_no_case("'cookie:\x20"),
        cookie_pairs,
        tag("'"),
    ))(input)?;
    Ok((input, cookies))
}

#[allow(dead_code)]
fn options_header_(input: &str) -> IResult<&str, (&str, &str)> {
    let (input, (_, k, _, v, _)) = tuple((
        alt((tag("-H\x20'"), tag("--header\x20'"))),
        key,
        tag(":\x20"),
        take_until("'"),
        tag("'"),
    ))(input)?;
    Ok((input, (k, v)))
}

#[allow(dead_code)]
fn options_data_raw(input: &str) -> IResult<&str, &str> {
    let (input, data) = delimited(tag("--data-raw\x20'"), take_until("'"), tag("'"))(input)?;
    Ok((input, data))
}

#[allow(dead_code)]
fn options_(input: &str) -> IResult<&str, (&str, Option<&str>)> {
    let (input, ((_, k), o_args)) = tuple((
        alt((tuple((tag("--"), key)), tuple((tag("-"), take(1usize))))),
        opt(delimited(tag("\x20'"), take_until("'"), tag("'"))),
    ))(input)?;
    Ok((input, (k, o_args)))
}

pub fn text_curl(input: &str) -> IResult<&str, Curl> {
    // curl <url>
    let (input, (_, _, _, _, url, _, _)) = tuple((
        multispace0,
        tag("curl"),
        multispace0,
        tag("'"),
        take_until("'"),
        tag("'"),
        opt(tag("\x20\\")),
    ))(input)?;
    let mut curl = Curl {
        url,
        options_header_cookies: HashMap::new(),
        options_headers_more: HashMap::new(),
        options_data_raw: "",
        options_more: HashMap::new(),
    };
    // [options..]
    let (input, opts) = separated_list0(
        tag("\x20"),
        tuple((
            opt(tag("\\\n")),
            multispace0,
            tuple((
                opt(options_header_cookie),
                opt(options_header_),
                opt(options_data_raw),
                opt(options_),
            )),
        )),
    )(input)?;
    for (_, _, o) in opts {
        if let Some(cookies) = o.0 {
            curl.options_header_cookies = cookies;
            continue;
        }
        if let Some(header) = o.1 {
            curl.options_headers_more.insert(header.0, header.1);
            continue;
        }
        if let Some(data) = o.2 {
            curl.options_data_raw = data;
            continue;
        }
        if let Some(options) = o.3 {
            match options.1 {
                Some(v) => curl.options_more.insert(options.0, v),
                None => curl.options_more.insert(options.0, ""),
            };
            continue;
        }
    }
    Ok((input, curl))
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::*;
    use serde_json;

    #[allow(dead_code)]
    fn d<T: Debug>(o: T) {
        println!("=> {:#?}", o);
    }

    #[test]
    fn test_options_data_raw() {
        let input = r#"--data-raw 'i=test&from=16264490298264&sign=ce3bfa052b83a7a6&lts=1626449029826&bv=186085bc402&doctype=json&v=2.1&' \"#;
        assert_eq!(options_data_raw(input), Ok((" \\", "i=test&from=16264490298264&sign=ce3bfa052b83a7a6&lts=1626449029826&bv=186085bc402&doctype=json&v=2.1&")));
    }

    #[test]
    fn test_options_() {
        let input = r#"-X 'GET' \"#;
        assert_eq!(options_(input), Ok(("\x20\\", ("X", Some("GET")))));
        let input = r#"--compressed"#;
        assert_eq!(options_(input), Ok(("", ("compressed", None))));
        let input = r#"-H 'Accept-Encoding: gzip, deflate, br' \"#;
        assert_eq!(
            options_(input),
            Ok(("\x20\\", ("H", Some("Accept-Encoding: gzip, deflate, br"))))
        );
    }

    #[test]
    fn test_cookie() -> Result<()> {
        let input = r#"-H 'Cookie: CGIC=Ij90ZXh0L2h0bWwsYXBwbG; NID=219=k9DN3imFveMmODL-Ji47zdfV6mSKlkKm; DV=03-vBWQ2RBEqsNFUD5' \"#;
        assert_eq!(
            options_header_cookie(input),
            Ok((
                "\x20\\",
                serde_json::from_str::<HashMap<&str, &str>>(
                    r#"{
                    "DV": "03-vBWQ2RBEqsNFUD5",
                    "CGIC": "Ij90ZXh0L2h0bWwsYXBwbG",
                    "NID": "219=k9DN3imFveMmODL-Ji47zdfV6mSKlkKm"
                }"#
                )?
            ))
        );
        Ok(())
    }

    #[test]
    fn test_options_header_() {
        let input = r#"-H 'Accept: */*'"#;
        assert_eq!(options_header_(input), Ok(("", ("Accept", "*/*"))));

        let input = r#"-H 'User-Agent: Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.114 Safari/537.36'"#;
        assert_eq!(options_header_(input), Ok(("", ("User-Agent", "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.114 Safari/537.36"))));

        let input =
            r#"-H 'sec-ch-ua: " Not;A Brand";v="99", "Google Chrome";v="91", "Chromium";v="91"' \"#;
        assert_eq!(
            options_header_(input),
            Ok((
                "\x20\\",
                (
                    "sec-ch-ua",
                    "\" Not;A Brand\";v=\"99\", \"Google Chrome\";v=\"91\", \"Chromium\";v=\"91\""
                )
            ))
        );
    }

    #[test]
    fn test_cp_from_safari() -> Result<()> {
        let input = r#"curl 'https://www.google.com/search?q=Q&source=hp&ei=R0D8YImOFpJgI&iflsig=AINFCbYAAAxk_1clsqfWaCx&oq=Q&gs_lcp=CgdndQdnd3Mtd2l6sAEA&sclient=gws-wiz&ved=0ahUKJ1N_ekPzUDCAs&uact=5' \
        -X 'GET' \
        -H 'Cookie: NID=219=afff42rGSsJ_ci7v87s_GmpKS24k-d-Gc9p6RUa_79ktj-HCqJX3iu3rEZgSYLikPThAxI' \
        -H 'Accept: text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8' \
        -H 'Accept-Encoding: gzip, deflate, br' \
        -H 'Host: www.google.com' \
        -H 'User-Agent: Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/14.1.2 Safari/605.1.15' \
        -H 'Accept-Language: en-US' \
        -H 'Referer: https://www.google.com/' \
        -H 'Connection: keep-alive'"#;
        assert_eq!(text_curl(input)?.1, Curl {
            url: "https://www.google.com/search?q=Q&source=hp&ei=R0D8YImOFpJgI&iflsig=AINFCbYAAAxk_1clsqfWaCx&oq=Q&gs_lcp=CgdndQdnd3Mtd2l6sAEA&sclient=gws-wiz&ved=0ahUKJ1N_ekPzUDCAs&uact=5",
            options_header_cookies: serde_json::from_str::<HashMap<&str, &str>>(r#"{
                "NID": "219=afff42rGSsJ_ci7v87s_GmpKS24k-d-Gc9p6RUa_79ktj-HCqJX3iu3rEZgSYLikPThAxI"
            }"#)?,
            options_headers_more: serde_json::from_str::<HashMap<&str, &str>>(r#"{
                "Accept": "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
                "Accept-Encoding": "gzip, deflate, br",
                "Host": "www.google.com",
                "Referer": "https://www.google.com/",
                "Connection": "keep-alive",
                "User-Agent": "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/14.1.2 Safari/605.1.15",
                "Accept-Language": "en-US"
            }"#)?,
            options_data_raw: "",
            options_more: serde_json::from_str::<HashMap<&str, &str>>(r#"{
                "X": "GET"
            }"#)?,
        });
        Ok(())
    }

    #[test]
    fn test_cp_from_chrome() -> Result<()> {
        let input = r#"curl 'https://www.google.com/search?q=Tokyo&rlz=1C5CHFA_enJP651JP651&oq=Tokyo&aqs=chrome..69i57j69i65.262j0j1&sourceid=chrome&ie=UTF-8' \
  -H 'authority: www.google.com' \
  -H 'cache-control: max-age=0' \
  -H 'sec-ch-ua-mobile: ?0' \
  -H 'upgrade-insecure-requests: 1' \
  -H 'user-agent: Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.164 Safari/537.36' \
  -H 'accept: text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8,application/signed-exchange;v=b3;q=0.9' \
  -H 'sec-fetch-site: same-origin' \
  -H 'sec-fetch-mode: navigate' \
  -H 'sec-fetch-user: ?1' \
  -H 'sec-fetch-dest: document' \
  -H 'accept-language: en-US,en;q=0.9' \
  -H 'cookie: CGIC=d2VicCxpbWFnZS9hc; NID=219=3XwFj5FYc2Jtkwl5K-QM2cWdxv8Am9t14-zH1QzxtHWEUT3BMg; DV=kyQKkk0J-HU_sc0eciTCQs_p7gJQEAAAA' \
  --compressed"#;
        assert_eq!(text_curl(input)?.1, Curl {
            url: "https://www.google.com/search?q=Tokyo&rlz=1C5CHFA_enJP651JP651&oq=Tokyo&aqs=chrome..69i57j69i65.262j0j1&sourceid=chrome&ie=UTF-8",
            options_header_cookies: serde_json::from_str::<HashMap<&str, &str>>(r#"{
                "DV": "kyQKkk0J-HU_sc0eciTCQs_p7gJQEAAAA",
                "CGIC": "d2VicCxpbWFnZS9hc",
                "NID": "219=3XwFj5FYc2Jtkwl5K-QM2cWdxv8Am9t14-zH1QzxtHWEUT3BMg"
            }"#)?,
            options_headers_more: serde_json::from_str::<HashMap<&str, &str>>(r#"{
                "sec-ch-ua-mobile": "?0",
                "user-agent": "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.164 Safari/537.36",
                "sec-fetch-user": "?1",
                "sec-fetch-site": "same-origin",
                "accept-language": "en-US,en;q=0.9",
                "authority": "www.google.com",
                "cache-control": "max-age=0",
                "sec-fetch-mode": "navigate",
                "accept": "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8,application/signed-exchange;v=b3;q=0.9",
                "sec-fetch-dest": "document",
                "upgrade-insecure-requests": "1"
            }"#)?,
            options_data_raw: "",
            options_more: serde_json::from_str::<HashMap<&str, &str>>(r#"{
                "compressed": ""
            }"#)?,
        });
        Ok(())
    }

    #[test]
    fn test_cp_from_firefox() -> Result<()> {
        let input = r#"curl 'https://www.google.com.hk/complete/search?q=Q&cp=0&client=gws-wiz&xssi=t&gs_ri=gws-wiz&hl=en-US&authuser=0&pq=ss&psi=sTPLuZr7wPvq-2wA0.162687873&ofp=EAE&dpr=2' -H 'User-Agent: Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:90.0) Gecko/20100101 Firefox/90.0' -H 'Accept: */*' -H 'Accept-Language: en-US,zh;q=0.8,zh-TW;q=0.7,zh-HK;q=0.5,en-US;q=0.3,en;q=0.2' --compressed -H 'Referer: https://www.google.com.hk/' -H 'Connection: keep-alive' -H 'Cookie: CGIC=Ikp0ZovKjtxPTAuOA; NID=219=tX80mwiQ4hbLDx-4wZuC8ySqLp1VLs; DV=E8Xt0D6kUcxCZ-YrBfXaKEAAAA' -H 'Sec-Fetch-Dest: empty' -H 'Sec-Fetch-Mode: cors' -H 'Sec-Fetch-Site: same-origin' -H 'TE: trailers'"#;
        assert_eq!(text_curl(input)?.1, Curl {
            url: "https://www.google.com.hk/complete/search?q=Q&cp=0&client=gws-wiz&xssi=t&gs_ri=gws-wiz&hl=en-US&authuser=0&pq=ss&psi=sTPLuZr7wPvq-2wA0.162687873&ofp=EAE&dpr=2",
            options_header_cookies: serde_json::from_str::<HashMap<&str, &str>>(r#"{
                "CGIC": "Ikp0ZovKjtxPTAuOA",
                "DV": "E8Xt0D6kUcxCZ-YrBfXaKEAAAA",
                "NID": "219=tX80mwiQ4hbLDx-4wZuC8ySqLp1VLs"
            }"#)?,
            options_headers_more: serde_json::from_str::<HashMap<&str, &str>>(r#"{
                "TE": "trailers",
                "Sec-Fetch-Mode": "cors",
                "Referer": "https://www.google.com.hk/",
                "User-Agent": "Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:90.0) Gecko/20100101 Firefox/90.0",
                "Accept-Language": "en-US,zh;q=0.8,zh-TW;q=0.7,zh-HK;q=0.5,en-US;q=0.3,en;q=0.2",
                "Connection": "keep-alive",
                "Sec-Fetch-Dest": "empty",
                "Sec-Fetch-Site": "same-origin",
                "Accept": "*/*"
            }"#)?,
            options_data_raw: "",
            options_more: serde_json::from_str::<HashMap<&str, &str>>(r#"{
                "compressed": ""
            }"#)?,
        });
        Ok(())
    }
}
