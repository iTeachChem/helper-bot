use std::time::Duration;

use poise::serenity_prelude as serenity;
use poise::CreateReply;

use super::{check_cooldown, parse_member};
use crate::render::{lb as lb_card, stats::StatsCard};
use crate::{Context, Error};

const COOLDOWN: Duration = Duration::from_secs(10);

fn fmt_time(secs: f64) -> String {
    let h = (secs / 3600.0).floor() as i64;
    let m = ((secs % 3600.0) / 60.0).floor() as i64;
    let s = secs % 60.0;
    if h != 0 {
        format!("{h}h {m}m")
    } else if m != 0 {
        format!("{m}m {s:.0}s")
    } else {
        format!("{s:.2}s")
    }
}

async fn fetch_avatar(http: &reqwest::Client, url: &str) -> Option<Vec<u8>> {
    match http.get(url).send().await {
        Ok(r) if r.status().is_success() => r.bytes().await.ok().map(|b| b.to_vec()),
        Ok(_) => None,
        Err(e) => {
            tracing::warn!("avatar fetch failed: {e}");
            None
        }
    }
}

fn leaderboard_button(ctx: Context<'_>) -> serenity::CreateActionRow {
    serenity::CreateActionRow::Buttons(vec![
        serenity::CreateButton::new_link(&ctx.data().config.urls.site_url).label("View Full Leaderboard"),
    ])
}

/// show quiz & doubt stats for you or another member
#[poise::command(prefix_command)]
pub async fn stats(ctx: Context<'_>, #[rest] who: Option<String>) -> Result<(), Error> {
    let target = match who.as_deref().and_then(|s| s.split_whitespace().next()) {
        Some(token) => match parse_member(ctx, token).await {
            Some(m) => m,
            None => {
                ctx.say("couldn't find that user.").await?;
                return Ok(());
            }
        },
        None => match ctx.author_member().await {
            Some(m) => m.into_owned(),
            None => return Ok(()),
        },
    };
    if !check_cooldown(ctx, "stats", COOLDOWN).await {
        return Ok(());
    }

    let row = match ctx.data().db.get_user_with_ranks(target.user.id.get()).await {
        Ok(row) => row,
        Err(e) => {
            tracing::error!("stats db error {}: {e}", target.user.id);
            ctx.say("couldn't fetch stats right now, try again later.").await?;
            return Ok(());
        }
    };
    let Some(row) = row else {
        ctx.say(format!("no stats found for **{}**.", target.display_name())).await?;
        return Ok(());
    };

    let get = |k: &str| row.get(k).cloned().unwrap_or(crate::db::Value::Null);
    let attempted = get("questions_attempted").as_i64();
    let solved = get("questions_solved").as_i64();
    let skipped = get("questions_skipped").as_i64();
    let points = get("points").as_f64() as i64;
    let total_secs = get("total_time_taken").as_f64();
    let rank = |k: &str| match get(k) {
        crate::db::Value::Null => "n/a".to_string(),
        v => format!("#{}", v.as_i64()),
    };
    let per_attempt = |v: f64| if attempted > 0 { v / attempted as f64 } else { 0.0 };

    let server_name = ctx.guild().map(|g| g.name.clone()).unwrap_or_else(|| "Direct Message".into());
    let card = StatsCard {
        username: target.display_name().to_string(),
        server_name,
        quiz_rank: rank("quiz_rank"),
        doubts_rank: rank("doubts_rank"),
        attempted,
        solved,
        skipped,
        points: (points != 0).then_some(points),
        total_time_str: (total_secs != 0.0).then(|| fmt_time(total_secs)),
        doubts_solved: get("doubts_solved").as_i64(),
        accuracy: per_attempt(solved as f64) * 100.0,
        avg_time: per_attempt(total_secs),
    };

    let _typing = ctx.channel_id().start_typing(&ctx.serenity_context().http);
    let avatar_url = target.face().replace("size=1024", "size=256");
    let avatar = fetch_avatar(&ctx.data().http, &avatar_url).await;
    let png = match tokio::task::spawn_blocking(move || crate::render::stats::render(avatar.as_deref(), &card)).await {
        Ok(Ok(png)) => png,
        Ok(Err(e)) => return render_failed(ctx, "stats", &target.user.id.to_string(), &e.to_string()).await,
        Err(e) => return render_failed(ctx, "stats", &target.user.id.to_string(), &e.to_string()).await,
    };

    ctx.send(
        CreateReply::default()
            .attachment(serenity::CreateAttachment::bytes(png, "stats.png"))
            .components(vec![leaderboard_button(ctx)]),
    )
    .await?;
    Ok(())
}

async fn render_failed(ctx: Context<'_>, what: &str, id: &str, err: &str) -> Result<(), Error> {
    tracing::error!("{what} render error {id}: {err}");
    let msg = if what == "lb" { "couldn't generate leaderboard card, try again later." } else { "couldn't generate stats card, try again later." };
    ctx.say(msg).await?;
    Ok(())
}

/// leaderboard — `+lb doubts` or `+lb quiz`
#[poise::command(prefix_command)]
pub async fn lb(ctx: Context<'_>, #[rest] board: Option<String>) -> Result<(), Error> {
    let Some(board) = board.as_deref().and_then(|s| s.split_whitespace().next()).map(str::to_lowercase) else {
        ctx.say("usage: `+lb doubts` or `+lb quiz`").await?;
        return Ok(());
    };
    let column = match board.as_str() {
        "doubts" => "doubts_solved",
        "quiz" => "questions_solved",
        _ => {
            ctx.say("unknown leaderboard — use `+lb doubts` or `+lb quiz`.").await?;
            return Ok(());
        }
    };
    if !check_cooldown(ctx, "lb", COOLDOWN).await {
        return Ok(());
    }

    let rows = match ctx.data().db.get_leaderboard(column, &ctx.data().config.excluded, 10).await {
        Ok(rows) => rows,
        Err(e) => {
            tracing::error!("lb db error {board}: {e}");
            ctx.say("couldn't fetch leaderboard, try again later.").await?;
            return Ok(());
        }
    };
    if rows.is_empty() {
        ctx.say("no data yet.").await?;
        return Ok(());
    }

    let card_rows: Vec<lb_card::LbRow> = rows
        .iter()
        .map(|r| lb_card::LbRow {
            username: r.get("username").map_or("", |v| v.as_str()).to_string(),
            value: r.get(column).map_or(0, |v| v.as_i64()),
        })
        .collect();

    let _typing = ctx.channel_id().start_typing(&ctx.serenity_context().http);
    let b = board.clone();
    let png = match tokio::task::spawn_blocking(move || lb_card::render(&b, &card_rows)).await {
        Ok(Ok(png)) => png,
        Ok(Err(e)) => return render_failed(ctx, "lb", &board, &e.to_string()).await,
        Err(e) => return render_failed(ctx, "lb", &board, &e.to_string()).await,
    };

    ctx.send(
        CreateReply::default()
            .attachment(serenity::CreateAttachment::bytes(png, format!("lb_{board}.png")))
            .components(vec![leaderboard_button(ctx)]),
    )
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::fmt_time;

    #[test]
    fn formats_time_like_python() {
        assert_eq!(fmt_time(8040.0), "2h 14m");
        assert_eq!(fmt_time(125.0), "2m 5s");
        assert_eq!(fmt_time(4.256), "4.26s");
    }
}
