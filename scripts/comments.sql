SELECT
	"User"."username" AS "channel",
	"Video"."id" AS "video_id",
	"Comment"."id" AS "comment_id",
	"Author"."username" AS "commenter",
	"Comment"."createdAt" AS "created_at",
	"Comment"."text" AS "text"
FROM "Comment"
	LEFT JOIN "User" "Author"
	ON "Comment"."userId" = "Author"."id"
	LEFT JOIN "Video"
	ON "Video"."id" = "Comment"."videoId"
	LEFT JOIN "User"
	ON "Video"."authorId" = "User"."id"
	ORDER BY "Comment"."createdAt" DESC;
